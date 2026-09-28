//! Private staging and validated, no-clobber publication for PSD layer PNGs.
use super::{
    ENGINE_ID, REQUEST_SEQUENCE, canonical_input, decode, descriptor, execution_error, invoke,
    map_manager_error, normalize_command,
};
use std::{
    fs,
    io::{self, BufReader},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::{Duration, Instant},
};
use yu_capability_psd::{
    PSD_CONTRACT_VERSION, PSD_LAYER_EXPORT, PsdLayerExportRequest, PsdLayerExportResult,
    PsdLayerId, layer_export_engine_request,
};
use yu_core::{EngineDescriptor, EngineState, ErrorCode, ResultEnvelope, YuError};
use yu_engine_api::ExternalEngineRequest;
use yu_engine_manager::{EngineManager, ManagedEngineCommand};

const MAX_RGBA_BYTES: usize = 256 * 1024 * 1024;
const MAX_PNG_BYTES: u64 = 320 * 1024 * 1024;

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;

pub fn export_layer(
    manager: &EngineManager,
    input: &Path,
    layer_id: PsdLayerId,
    output: &Path,
    requested_engine: Option<&str>,
    timeout: Duration,
) -> Result<ResultEnvelope<PsdLayerExportResult>, YuError> {
    if requested_engine.is_some_and(|id| id != ENGINE_ID) {
        return Err(YuError::new(
            ErrorCode::EngineUnavailable,
            "only Managed ag-psd is wired for PSD export; no automatic fallback",
        ));
    }
    if timeout.is_zero() || timeout > Duration::from_secs(3600) {
        return Err(YuError::new(
            ErrorCode::InvalidArgument,
            "PSD timeout must be greater than zero and at most 3600 seconds",
        ));
    }
    let input = canonical_input(input)?;
    let destination = destination(output)?;
    let mut command = manager.active_command(&descriptor()).map_err(map_manager_error)?
        .ok_or_else(|| YuError::new(ErrorCode::EngineUnavailable, "ag-psd is not installed or not active; explicitly install and activate an export-capable version"))?;
    let selected = EngineDescriptor {
        state: EngineState::Ready,
        version: Some(command.version.clone()),
        capabilities: command.capabilities.clone(),
        ..descriptor()
    };
    export_selected_with(
        &mut command,
        &selected,
        input,
        layer_id,
        &destination,
        timeout,
        invoke,
    )
}

// Keep private staging and publication behind the settled transport result. The injected
// callable is private and enables tests without public fault flags or mutable global state.
fn export_selected_with(
    command: &mut ManagedEngineCommand,
    selected: &EngineDescriptor,
    input: String,
    layer_id: PsdLayerId,
    destination: &Path,
    timeout: Duration,
    invoke_engine: impl FnOnce(
        &ManagedEngineCommand,
        &ExternalEngineRequest<PsdLayerExportRequest>,
        Duration,
    ) -> Result<(serde_json::Value, Vec<String>), YuError>,
) -> Result<ResultEnvelope<PsdLayerExportResult>, YuError> {
    let operation = || -> Result<ResultEnvelope<PsdLayerExportResult>, YuError> {
        if !command.capabilities.iter().any(|id| id == PSD_LAYER_EXPORT) {
            return Err(YuError::new(
                ErrorCode::EngineIncompatible,
                "active ag-psd version does not declare psd.layer.export; explicitly install and activate the newer package",
            ));
        }
        normalize_command(command)?;
        let stage = tempfile::Builder::new()
            .prefix(".yu-psd-export-")
            .tempdir_in(destination.parent().expect("validated output parent"))
            .map_err(|e| execution_error(format!("cannot create private export staging: {e}")))?;
        let staged_path = stage.path().join("layer.png");
        let staged_text = staged_path
            .to_str()
            .ok_or_else(|| YuError::new(ErrorCode::InvalidInput, "staging path is not UTF-8"))?;
        let request = layer_export_engine_request(
            format!(
                "yu-export-{}-{}",
                std::process::id(),
                REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ),
            input,
            layer_id.clone(),
            staged_text,
        );
        let started = Instant::now();
        let (value, warnings) = invoke_engine(command, &request, timeout)?;
        let mut result: PsdLayerExportResult = decode(value)?;
        if result.contract_version != PSD_CONTRACT_VERSION
            || result.layer_id != layer_id
            || result.output_path != staged_text
        {
            return Err(execution_error(
                "export result contract, layer ID, or staging path mismatch",
            ));
        }
        verify_png(&staged_path, result.width, result.height)?;
        if started.elapsed() >= timeout {
            return Err(execution_error(
                "export timed out before publishing verified PNG",
            ));
        }
        publish(&staged_path, destination)?;
        result.output_path = destination
            .to_str()
            .expect("validated UTF-8 output")
            .to_owned();
        let mut envelope = ResultEnvelope::new(PSD_LAYER_EXPORT, result).with_engine(selected);
        envelope.warnings = warnings;
        if let Err(error) = stage.close() {
            envelope.warnings.push(format!(
                "export succeeded but staging cleanup failed: {error}"
            ));
        }
        Ok(envelope)
    };
    operation().map_err(|error| error.with_engine(selected))
}

fn destination(output: &Path) -> Result<PathBuf, YuError> {
    fn invalid(message: impl Into<String>) -> YuError {
        YuError::new(ErrorCode::InvalidArgument, message)
    }
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| invalid("output must have a UTF-8 filename"))?;
    if output.to_str().is_none() {
        return Err(invalid("output path must be UTF-8"));
    }
    #[cfg(windows)]
    if name.contains(':') || name.ends_with('.') || name.ends_with(' ') {
        return Err(invalid(
            "Windows output must not use an alternate data stream or ambiguous suffix",
        ));
    }
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = fs::canonicalize(parent)
        .map_err(|e| invalid(format!("output parent must already exist: {e}")))?;
    if !parent.is_dir() {
        return Err(invalid("output parent must be a directory"));
    }
    let target = parent.join(name);
    match fs::symlink_metadata(&target) {
        Ok(_) => {
            return Err(YuError::new(
                ErrorCode::OutputConflict,
                format!("output already exists: {}", target.display()),
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(execution_error(format!(
                "cannot inspect output path: {error}"
            )));
        }
    }
    if !output
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("png"))
    {
        return Err(invalid("layer export output must use the .png extension"));
    }
    if target.to_str().is_none() {
        return Err(invalid("resolved output path must be UTF-8"));
    }
    Ok(target)
}

fn pixel_bytes(width: u32, height: u32) -> Result<usize, YuError> {
    let bytes = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4));
    match bytes {
        Some(bytes) if width > 0 && height > 0 && bytes <= MAX_RGBA_BYTES as u64 => {
            Ok(bytes as usize)
        }
        _ => Err(execution_error(
            "export dimensions exceed the 256 MiB RGBA8 limit or are empty",
        )),
    }
}

fn verify_png(path: &Path, width: u32, height: u32) -> Result<(), YuError> {
    let expected_bytes = pixel_bytes(width, height)?;
    let metadata = fs::symlink_metadata(path)
        .map_err(|e| execution_error(format!("export artifact is missing: {e}")))?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > MAX_PNG_BYTES {
        return Err(execution_error(
            "export artifact must be a bounded regular PNG file",
        ));
    }
    let file = fs::File::open(path)
        .map_err(|e| execution_error(format!("cannot open exported PNG: {e}")))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_limits(png::Limits {
        bytes: MAX_RGBA_BYTES,
    });
    let mut reader = decoder
        .read_info()
        .map_err(|e| execution_error(format!("invalid PNG header: {e}")))?;
    let info = reader.info();
    if info.width != width
        || info.height != height
        || info.bit_depth != png::BitDepth::Eight
        || info.color_type != png::ColorType::Rgba
        || info.animation_control.is_some()
        || info.frame_control.is_some()
    {
        return Err(execution_error(
            "export must be a static RGBA8 PNG matching the reported dimensions",
        ));
    }
    let mut pixels = vec![0; expected_bytes];
    let frame = reader
        .next_frame(&mut pixels)
        .map_err(|e| execution_error(format!("invalid PNG image data: {e}")))?;
    if frame.buffer_size() != expected_bytes {
        return Err(execution_error("PNG decoded size mismatch"));
    }
    reader
        .finish()
        .map_err(|e| execution_error(format!("invalid PNG trailer: {e}")))?;
    Ok(())
}

fn publish(staged: &Path, output: &Path) -> Result<(), YuError> {
    // Same-filesystem hard link is atomic and cannot replace an existing destination.
    fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(staged)
        .and_then(|file| file.sync_all())
        .map_err(|e| execution_error(format!("cannot sync exported PNG: {e}")))?;
    fs::hard_link(staged, output).map_err(|error| {
        if error.kind() == io::ErrorKind::AlreadyExists || fs::symlink_metadata(output).is_ok() {
            YuError::new(
                ErrorCode::OutputConflict,
                "output appeared before publication; existing data was not overwritten",
            )
        } else {
            execution_error(format!(
                "cannot atomically publish PNG; output filesystem must support hard links: {error}"
            ))
        }
    })
}
