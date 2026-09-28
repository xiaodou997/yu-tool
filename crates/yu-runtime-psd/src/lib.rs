//! Managed PSD routing and execution, independent of CLI parsing and rendering.
mod export;
pub use export::export_layer;
mod process;
mod validation;

use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use yu_capability_psd::{
    PSD_INSPECT, PSD_LAYER_INFO, PSD_LAYER_LIST, PSD_TREE, PsdDocumentInfo, PsdInspectResult,
    PsdLayerId, PsdLayerInfoResult, PsdLayerListResult, PsdTreeResult, inspect_engine_request,
    layer_info_engine_request, layer_list_engine_request, tree_engine_request,
};
use yu_core::{
    CapabilityDescriptor, EngineDescriptor, EngineProvider, EngineState, ErrorCode, ResultEnvelope,
    YuError,
};
use yu_engine_api::{
    ExternalEngineErrorCode, ExternalEngineRequest, ExternalEngineResponse,
    validate_external_response,
};
use yu_engine_manager::{EngineManager, ManagedEngineCommand, ManagerError};

pub const ENGINE_ID: &str = "ag-psd";
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
pub const READ_ONLY_CAPABILITIES: [(&str, &str); 4] = [
    (PSD_INSPECT, "Inspect PSD/PSB document metadata."),
    (PSD_TREE, "Read the canonical PSD/PSB layer tree."),
    (PSD_LAYER_LIST, "List canonical PSD/PSB layers."),
    (PSD_LAYER_INFO, "Inspect one PSD/PSB layer by stable ID."),
];
static REQUEST_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub enum ReadOperation {
    Inspect,
    Tree,
    LayerList,
    LayerInfo(PsdLayerId),
}

impl ReadOperation {
    pub fn capability(&self) -> &'static str {
        match self {
            Self::Inspect => PSD_INSPECT,
            Self::Tree => PSD_TREE,
            Self::LayerList => PSD_LAYER_LIST,
            Self::LayerInfo(_) => PSD_LAYER_INFO,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum PsdReadResult {
    Inspect(PsdInspectResult),
    Tree(PsdTreeResult),
    LayerList(PsdLayerListResult),
    LayerInfo(PsdLayerInfoResult),
}

impl PsdReadResult {
    pub fn document(&self) -> &PsdDocumentInfo {
        match self {
            Self::Inspect(result) => &result.document,
            Self::Tree(result) => &result.document,
            Self::LayerList(result) => &result.document,
            Self::LayerInfo(result) => &result.document,
        }
    }
}

fn descriptor() -> EngineDescriptor {
    EngineDescriptor {
        id: ENGINE_ID.to_owned(),
        display_name: "Managed ag-psd".to_owned(),
        provider: EngineProvider::Managed,
        state: EngineState::NotInstalled,
        version: None,
        capabilities: Vec::new(),
    }
}

/// Metadata-based availability; never starts, installs, or activates an engine.
/// Broken/inactive engines remain visible in Engine Manager diagnostics.
pub fn available_capabilities(manager: &EngineManager) -> Vec<CapabilityDescriptor> {
    let Ok(Some(mut command)) = manager.active_command(&descriptor()) else {
        return Vec::new();
    };
    if normalize_command(&mut command).is_err() {
        return Vec::new();
    }
    READ_ONLY_CAPABILITIES
        .iter()
        .chain(std::iter::once(&(
            yu_capability_psd::PSD_LAYER_EXPORT,
            "Export one stored 8-bit RGB layer bitmap as RGBA8 PNG.",
        )))
        .filter(|(id, _)| {
            command
                .capabilities
                .iter()
                .any(|capability| capability == id)
        })
        .map(|(id, summary)| CapabilityDescriptor {
            id: (*id).to_owned(),
            summary: (*summary).to_owned(),
            engines: vec![ENGINE_ID.to_owned()],
        })
        .collect()
}

pub fn execute(
    manager: &EngineManager,
    operation: ReadOperation,
    input: &Path,
    requested_engine: Option<&str>,
    timeout: Duration,
) -> Result<ResultEnvelope<PsdReadResult>, YuError> {
    if let Some(id) = requested_engine
        && id != ENGINE_ID
    {
        return Err(YuError::new(
            ErrorCode::EngineUnavailable,
            format!(
                "PSD engine {id} is not wired into this build; only Managed {ENGINE_ID} is available, with no automatic fallback"
            ),
        ));
    }
    if timeout.is_zero() || timeout > Duration::from_secs(3600) {
        return Err(YuError::new(
            ErrorCode::InvalidArgument,
            "PSD timeout must be greater than zero and at most 3600 seconds",
        ));
    }
    let input_path = canonical_input(input)?;
    let mut command = manager.active_command(&descriptor()).map_err(map_manager_error)?.ok_or_else(|| {
        YuError::new(ErrorCode::EngineUnavailable,
            "ag-psd is not installed or not active; explicitly install a trusted manifest with `yu engine install --manifest <file>` and activate its version with `yu engine activate ag-psd <version>`")
    })?;
    let selected = EngineDescriptor {
        state: EngineState::Ready,
        version: Some(command.version.clone()),
        capabilities: command.capabilities.clone(),
        ..descriptor()
    };
    execute_selected(&mut command, &operation, input_path, timeout)
        .map(|(result, warnings)| {
            let mut envelope =
                ResultEnvelope::new(operation.capability(), result).with_engine(&selected);
            envelope.warnings = warnings;
            envelope
        })
        .map_err(|error| error.with_engine(&selected))
}

fn execute_selected(
    command: &mut ManagedEngineCommand,
    operation: &ReadOperation,
    input: String,
    timeout: Duration,
) -> Result<(PsdReadResult, Vec<String>), YuError> {
    if !command
        .capabilities
        .iter()
        .any(|id| id == operation.capability())
    {
        return Err(YuError::new(
            ErrorCode::EngineIncompatible,
            format!(
                "active ag-psd version {} does not declare {}",
                command.version,
                operation.capability()
            ),
        ));
    }
    normalize_command(command)?;
    let request_id = format!(
        "yu-psd-{}-{}",
        std::process::id(),
        REQUEST_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let request: ExternalEngineRequest<Value> = match operation {
        ReadOperation::Inspect => erase_payload(inspect_engine_request(request_id, input))?,
        ReadOperation::Tree => erase_payload(tree_engine_request(request_id, input))?,
        ReadOperation::LayerList => erase_payload(layer_list_engine_request(request_id, input))?,
        ReadOperation::LayerInfo(id) => {
            erase_payload(layer_info_engine_request(request_id, input, id.clone()))?
        }
    };
    let (value, warnings) = invoke(command, &request, timeout)?;
    Ok((decode_result(operation, value)?, warnings))
}

fn invoke<T: Serialize>(
    command: &ManagedEngineCommand,
    request: &ExternalEngineRequest<T>,
    timeout: Duration,
) -> Result<(Value, Vec<String>), YuError> {
    request
        .validate_header()
        .map_err(|e| execution_error(e.to_string()))?;
    let bytes = serde_json::to_vec(&request).map_err(|e| execution_error(e.to_string()))?;
    let output = process::execute(command, bytes, timeout).map_err(execution_error)?;
    let response: ExternalEngineResponse<Value> =
        serde_json::from_slice(&output.stdout).map_err(|e| {
            execution_error(format!(
                "engine stdout is not one Protocol v1 response: {e}"
            ))
        })?;
    validate_external_response(request, &response).map_err(|e| execution_error(e.to_string()))?;
    match response {
        ExternalEngineResponse::Ok {
            result,
            mut warnings,
            ..
        } => {
            let diagnostics = String::from_utf8_lossy(&output.stderr);
            if !diagnostics.trim().is_empty() {
                warnings.push(format!("engine diagnostic: {}", diagnostics.trim()));
            }
            Ok((result, warnings))
        }
        ExternalEngineResponse::Error { error, .. } => {
            let code = match error.code {
                ExternalEngineErrorCode::InvalidArgument => ErrorCode::InvalidArgument,
                ExternalEngineErrorCode::InvalidInput => ErrorCode::InvalidInput,
                ExternalEngineErrorCode::UnsupportedCapability => ErrorCode::UnsupportedCapability,
                ExternalEngineErrorCode::EngineIncompatible => ErrorCode::EngineIncompatible,
                ExternalEngineErrorCode::ExecutionFailed => ErrorCode::ExecutionFailed,
            };
            Err(YuError::new(code, error.message))
        }
    }
}

fn erase_payload<T: Serialize>(
    request: ExternalEngineRequest<T>,
) -> Result<ExternalEngineRequest<Value>, YuError> {
    Ok(ExternalEngineRequest {
        protocol_version: request.protocol_version,
        request_id: request.request_id,
        capability: request.capability,
        payload: serde_json::to_value(request.payload)
            .map_err(|e| execution_error(e.to_string()))?,
    })
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, YuError> {
    serde_json::from_value(value)
        .map_err(|e| execution_error(format!("invalid PSD result schema: {e}")))
}

fn decode_result(operation: &ReadOperation, value: Value) -> Result<PsdReadResult, YuError> {
    let result = match operation {
        ReadOperation::Inspect => PsdReadResult::Inspect(decode(value)?),
        ReadOperation::Tree => PsdReadResult::Tree(decode(value)?),
        ReadOperation::LayerList => PsdReadResult::LayerList(decode(value)?),
        ReadOperation::LayerInfo(_) => PsdReadResult::LayerInfo(decode(value)?),
    };
    validation::validate(operation, &result)
        .map_err(|e| execution_error(format!("invalid PSD contract result: {e}")))?;
    Ok(result)
}

fn canonical_input(input: &Path) -> Result<String, YuError> {
    if input.to_str().is_none() {
        return Err(YuError::new(
            ErrorCode::InvalidInput,
            "PSD input path is not UTF-8 and cannot be represented by Protocol v1",
        ));
    }
    let path = fs::canonicalize(input).map_err(|e| {
        YuError::new(
            ErrorCode::InvalidInput,
            format!("cannot resolve PSD input {}: {e}", input.display()),
        )
    })?;
    let metadata = fs::metadata(&path).map_err(|e| {
        YuError::new(
            ErrorCode::InvalidInput,
            format!("cannot inspect PSD input: {e}"),
        )
    })?;
    if !metadata.is_file() {
        return Err(YuError::new(
            ErrorCode::InvalidInput,
            "PSD input must be a regular file",
        ));
    }
    path.to_str().map(str::to_owned).ok_or_else(|| {
        YuError::new(
            ErrorCode::InvalidInput,
            "PSD input path is not UTF-8 and cannot be represented by Protocol v1",
        )
    })
}

fn normalize_command(command: &mut ManagedEngineCommand) -> Result<(), YuError> {
    // Resolve before setting current_dir; YU_DATA_HOME is allowed to be relative.
    command.entrypoint = fs::canonicalize(&command.entrypoint).map_err(|e| {
        YuError::new(
            ErrorCode::EngineUnavailable,
            format!("cannot resolve active engine entrypoint: {e}"),
        )
    })?;
    command.working_dir = fs::canonicalize(&command.working_dir).map_err(|e| {
        YuError::new(
            ErrorCode::EngineUnavailable,
            format!("cannot resolve active engine directory: {e}"),
        )
    })?;
    #[cfg(windows)]
    if !command
        .entrypoint
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    {
        return Err(YuError::new(
            ErrorCode::EngineIncompatible,
            "Managed PSD entrypoint must be a native .exe, not a shell script",
        ));
    }
    Ok(())
}

fn map_manager_error(error: ManagerError) -> YuError {
    let code = match error {
        ManagerError::Incompatible(_) | ManagerError::Ownership(_) => ErrorCode::EngineIncompatible,
        ManagerError::NotInstalled(_) | ManagerError::State(_) | ManagerError::Io(_) => {
            ErrorCode::EngineUnavailable
        }
        _ => ErrorCode::ExecutionFailed,
    };
    YuError::new(code, format!("cannot use active Managed ag-psd: {error}"))
}

fn execution_error(message: impl Into<String>) -> YuError {
    YuError::new(ErrorCode::ExecutionFailed, message)
}
