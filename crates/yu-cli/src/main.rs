mod psd;

use clap::{Parser, Subcommand};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{ExitCode, Termination},
};
use yu_capability_image::{
    ConvertRequest, CropRequest, ImageEngine, ImageErrorKind, ImageOperationError, ResizeRequest,
    RotateRequest,
};
use yu_core::{
    EngineDescriptor, EngineProvider, EngineState, ErrorCode, ErrorEnvelope, ResultEnvelope,
    RuntimeRegistry, YuError,
};
use yu_engine_image_rs::{ENGINE_ID as RASTER_ENGINE_ID, RustImageEngine};
use yu_engine_manager::{
    EngineInstaller, EngineInventoryEntry, EngineManager, EngineManifest, HttpDownloader,
    LocalArchiveDownloader, ManagerError,
};

#[derive(Debug, Parser)]
#[command(
    name = "yu",
    version,
    about = "Lightweight local tool runtime for developers and AI agents"
)]
struct Cli {
    #[arg(long, global = true, help = "Emit machine-readable JSON")]
    json: bool,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Diagnose YuTool runtime and engine health.
    Doctor,
    /// List capabilities available on this machine.
    Capabilities,
    /// Inspect and manage engines.
    Engine {
        #[command(subcommand)]
        command: EngineCommand,
    },
    /// Inspect and transform raster images.
    Image {
        #[command(subcommand)]
        command: ImageCommand,
    },
    /// Read PSD/PSB metadata and layers through an activated managed engine.
    Psd {
        #[command(subcommand)]
        command: psd::PsdCommand,
    },
}

#[derive(Debug, Subcommand)]
enum EngineCommand {
    /// List discovered built-in, managed, and system engines.
    List,
    /// Inspect all discovered providers for an engine ID.
    Info { engine: String },
    /// Install a managed engine from a local manifest.
    Install {
        #[arg(long)]
        manifest: PathBuf,
        /// Use this local archive without network access; manifest verification still applies.
        #[arg(long)]
        archive: Option<PathBuf>,
    },
    /// List installed versions of a managed engine.
    Versions { engine: String },
    /// Activate an installed managed-engine version.
    Activate { engine: String, version: String },
    /// Deactivate a managed engine without removing installed versions.
    Deactivate { engine: String },
    /// Remove an inactive managed-engine version.
    Remove { engine: String, version: String },
}

#[derive(Debug, Subcommand)]
enum ImageCommand {
    /// Inspect image dimensions, format, and color information.
    Info {
        file: PathBuf,

        #[arg(long, help = "Use a specific engine instead of automatic resolution")]
        engine: Option<String>,
    },
    /// Resize an image to a new output file.
    Resize {
        input: PathBuf,

        #[arg(long)]
        width: Option<u32>,

        #[arg(long)]
        height: Option<u32>,

        #[arg(short, long)]
        output: PathBuf,

        #[arg(long, help = "Preview without writing a file")]
        dry_run: bool,

        #[arg(long, help = "Use a specific engine instead of automatic resolution")]
        engine: Option<String>,
    },
    /// Crop an in-bounds rectangle to a new image.
    Crop {
        input: PathBuf,
        #[arg(long)]
        x: u32,
        #[arg(long)]
        y: u32,
        #[arg(long)]
        width: u32,
        #[arg(long)]
        height: u32,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, help = "Preview without writing a file")]
        dry_run: bool,
        #[arg(long, help = "Use a specific engine instead of automatic resolution")]
        engine: Option<String>,
    },
    /// Rotate clockwise by 90, 180, or 270 degrees.
    Rotate {
        input: PathBuf,
        #[arg(long)]
        degrees: u16,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, help = "Preview without writing a file")]
        dry_run: bool,
        #[arg(long, help = "Use a specific engine instead of automatic resolution")]
        engine: Option<String>,
    },
    /// Convert between supported PNG, JPEG, and WebP formats.
    Convert {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long, help = "Preview without writing a file")]
        dry_run: bool,
        #[arg(long, help = "Use a specific engine instead of automatic resolution")]
        engine: Option<String>,
    },
}

fn main() -> impl Termination {
    let args: Vec<_> = std::env::args_os().collect();
    let requested_json = args
        .iter()
        .take_while(|arg| *arg != "--")
        .any(|arg| arg == "--json");
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(error) => {
            if error.use_stderr() && requested_json {
                render_error(
                    &YuError::new(ErrorCode::InvalidArgument, error.to_string()),
                    true,
                );
            } else {
                let _ = error.print();
            }
            return ExitCode::from(if error.use_stderr() { 2 } else { 0 });
        }
    };
    let json = cli.json;

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            render_error(&error, json);
            ExitCode::from(error.exit_code())
        }
    }
}

fn run(cli: Cli) -> Result<(), YuError> {
    let registry = RuntimeRegistry::bootstrap();

    match cli.command {
        Command::Doctor => render_doctor(&registry, cli.json),
        Command::Psd { command } => psd::run(command, cli.json),
        Command::Capabilities => {
            render_capabilities(&registry, cli.json);
            Ok(())
        }
        Command::Engine {
            command: EngineCommand::List,
        } => render_engines(&registry, cli.json),
        Command::Engine {
            command: EngineCommand::Info { engine },
        } => render_engine_info(&registry, &engine, cli.json),
        Command::Engine {
            command: EngineCommand::Install { manifest, archive },
        } => render_engine_install(&manifest, archive.as_deref(), cli.json),
        Command::Engine {
            command: EngineCommand::Versions { engine },
        } => render_engine_versions(&engine, cli.json),
        Command::Engine {
            command: EngineCommand::Activate { engine, version },
        } => render_engine_activate(&engine, &version, cli.json),
        Command::Engine {
            command: EngineCommand::Deactivate { engine },
        } => render_engine_deactivate(&engine, cli.json),
        Command::Engine {
            command: EngineCommand::Remove { engine, version },
        } => render_engine_remove(&engine, &version, cli.json),
        Command::Image {
            command: ImageCommand::Info { file, engine },
        } => render_image_info(&registry, file, engine.as_deref(), cli.json),
        Command::Image {
            command:
                ImageCommand::Resize {
                    input,
                    width,
                    height,
                    output,
                    dry_run,
                    engine,
                },
        } => render_image_resize(
            &registry,
            ResizeRequest {
                input,
                output,
                width,
                height,
                dry_run,
            },
            engine.as_deref(),
            cli.json,
        ),
        Command::Image {
            command:
                ImageCommand::Crop {
                    input,
                    x,
                    y,
                    width,
                    height,
                    output,
                    dry_run,
                    engine,
                },
        } => render_image_crop(
            &registry,
            CropRequest {
                input,
                output,
                x,
                y,
                width,
                height,
                dry_run,
            },
            engine.as_deref(),
            cli.json,
        ),
        Command::Image {
            command:
                ImageCommand::Rotate {
                    input,
                    degrees,
                    output,
                    dry_run,
                    engine,
                },
        } => render_image_rotate(
            &registry,
            RotateRequest {
                input,
                output,
                degrees,
                dry_run,
            },
            engine.as_deref(),
            cli.json,
        ),
        Command::Image {
            command:
                ImageCommand::Convert {
                    input,
                    output,
                    dry_run,
                    engine,
                },
        } => render_image_convert(
            &registry,
            ConvertRequest {
                input,
                output,
                dry_run,
            },
            engine.as_deref(),
            cli.json,
        ),
    }
}

fn render_doctor(registry: &RuntimeRegistry, json: bool) -> Result<(), YuError> {
    let inventory = discover_inventory(registry)?;
    let descriptors = inventory
        .iter()
        .map(EngineInventoryEntry::descriptor)
        .collect::<Vec<_>>();
    let mut report = registry.doctor_report_with_engines(&descriptors);
    report.capabilities = effective_capabilities(registry).len();
    let warnings = inventory_warnings(&inventory);

    if json {
        let mut envelope = ResultEnvelope::new("runtime.doctor", report);
        for warning in warnings {
            envelope = envelope.with_warning(warning);
        }
        print_json(&envelope);
        return Ok(());
    }

    println!("YuTool {}", report.version);
    println!(
        "Status: {}",
        if report.healthy {
            "healthy"
        } else {
            "degraded"
        }
    );
    println!("Platform: {}/{}", report.platform.os, report.platform.arch);
    println!("Capabilities: {}", report.capabilities);
    println!(
        "Engines: {} total, {} ready ({} built-in / {} managed / {} system)",
        report.engines.total,
        report.engines.ready,
        report.engines.built_in,
        report.engines.managed,
        report.engines.system
    );

    for warning in warnings {
        eprintln!("warning: {warning}");
    }

    Ok(())
}

fn effective_capabilities(registry: &RuntimeRegistry) -> Vec<yu_core::CapabilityDescriptor> {
    let mut capabilities = registry.capabilities().to_vec();
    if let Ok(manager) = EngineManager::discover() {
        capabilities.extend(yu_runtime_psd::available_capabilities(&manager));
    }
    capabilities
}

fn render_capabilities(registry: &RuntimeRegistry, json: bool) {
    let capabilities = effective_capabilities(registry);
    if json {
        print_json(&ResultEnvelope::new("runtime.capabilities", &capabilities));
        return;
    }

    println!("{:<24} {:<16} DESCRIPTION", "CAPABILITY", "ENGINES");
    for capability in &capabilities {
        println!(
            "{:<24} {:<16} {}",
            capability.id,
            capability.engines.join(","),
            capability.summary
        );
    }
}

fn render_engines(registry: &RuntimeRegistry, json: bool) -> Result<(), YuError> {
    let inventory = discover_inventory(registry)?;

    if json {
        print_json(&ResultEnvelope::new("engine.list", inventory));
        return Ok(());
    }

    println!(
        "{:<16} {:<10} {:<12} {:<16} {:<16} EXECUTABLE",
        "ENGINE", "PROVIDER", "STATE", "VERSION", "ACTIVE"
    );
    for engine in inventory {
        println!(
            "{:<16} {:<10} {:<12} {:<16} {:<16} {}",
            engine.id,
            engine.provider,
            engine.state,
            engine.version.as_deref().unwrap_or("-"),
            engine.active_version.as_deref().unwrap_or("-"),
            engine
                .executable
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_owned())
        );
    }

    Ok(())
}

fn render_engine_info(
    registry: &RuntimeRegistry,
    engine_id: &str,
    json: bool,
) -> Result<(), YuError> {
    let matches = discover_inventory(registry)?
        .into_iter()
        .filter(|engine| engine.id == engine_id)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        return Err(YuError::new(
            ErrorCode::EngineUnavailable,
            format!("engine is not discovered: {engine_id}"),
        ));
    }

    if json {
        print_json(&ResultEnvelope::new("engine.info", matches));
        return Ok(());
    }

    for (index, engine) in matches.iter().enumerate() {
        if index > 0 {
            println!();
        }
        println!("Engine: {}", engine.id);
        println!("Name: {}", engine.display_name);
        println!("Provider: {}", engine.provider);
        println!("State: {}", engine.state);
        println!("Version: {}", engine.version.as_deref().unwrap_or("-"));
        println!(
            "Active version: {}",
            engine.active_version.as_deref().unwrap_or("-")
        );
        println!(
            "Installed versions: {}",
            if engine.installed_versions.is_empty() {
                "-".to_owned()
            } else {
                engine.installed_versions.join(", ")
            }
        );
        println!(
            "Executable: {}",
            engine
                .executable
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_owned())
        );
        println!(
            "Capabilities: {}",
            if engine.capabilities.is_empty() {
                "-".to_owned()
            } else {
                engine.capabilities.join(", ")
            }
        );
        for warning in &engine.warnings {
            println!("Warning: {warning}");
        }
    }

    Ok(())
}

fn discover_inventory(registry: &RuntimeRegistry) -> Result<Vec<EngineInventoryEntry>, YuError> {
    let manager = EngineManager::discover().map_err(map_manager_error)?;
    manager
        .discover_inventory(registry.engines())
        .map_err(map_manager_error)
}

fn inventory_warnings(inventory: &[EngineInventoryEntry]) -> Vec<String> {
    inventory
        .iter()
        .flat_map(|engine| {
            engine
                .warnings
                .iter()
                .map(|warning| format!("{} [{}]: {warning}", engine.id, engine.provider))
        })
        .collect()
}

fn render_engine_install(
    manifest_path: &Path,
    archive: Option<&Path>,
    json: bool,
) -> Result<(), YuError> {
    let manifest = read_manifest(manifest_path)?;
    let manager = EngineManager::discover().map_err(map_manager_error)?;
    let receipt = if let Some(archive) = archive {
        let package = manifest.package_for(manager.target()).ok_or_else(|| {
            YuError::new(
                ErrorCode::EngineIncompatible,
                "manifest has no package for this platform",
            )
        })?;
        let downloader = LocalArchiveDownloader::new(archive, &package.url);
        EngineInstaller::new(manager, downloader).install(&manifest)
    } else {
        let downloader = HttpDownloader::new().map_err(map_manager_error)?;
        EngineInstaller::new(manager, downloader).install(&manifest)
    }
    .map_err(map_manager_error)?;

    if json {
        print_json(&ResultEnvelope::new("engine.install", receipt));
    } else {
        println!("Installed {} {}", receipt.engine_id, receipt.version);
        println!("Target: {}/{}", receipt.target_os, receipt.target_arch);
        println!("Entrypoint: {}", receipt.entrypoint.display());
        println!("Active version: unchanged");
    }
    Ok(())
}

fn render_engine_versions(engine_id: &str, json: bool) -> Result<(), YuError> {
    let manager = EngineManager::discover().map_err(map_manager_error)?;
    let descriptor = managed_descriptor(engine_id);
    let versions = manager
        .list_managed_versions(&descriptor)
        .map_err(map_manager_error)?;

    if json {
        print_json(&ResultEnvelope::new("engine.versions", versions));
    } else if versions.is_empty() {
        println!("No managed versions installed for {engine_id}");
    } else {
        println!("{:<16} {:<8} PATH", "VERSION", "ACTIVE");
        for version in versions {
            println!(
                "{:<16} {:<8} {}",
                version.version,
                if version.active { "yes" } else { "no" },
                version.path.display()
            );
        }
    }
    Ok(())
}

fn render_engine_activate(engine_id: &str, version: &str, json: bool) -> Result<(), YuError> {
    let manager = EngineManager::discover().map_err(map_manager_error)?;
    let descriptor = managed_descriptor(engine_id);
    let receipt = manager
        .activate_version(&descriptor, version)
        .map_err(map_manager_error)?;

    if json {
        print_json(&ResultEnvelope::new("engine.activate", receipt));
    } else {
        println!(
            "Active {}: {}{}",
            receipt.engine_id,
            receipt.active_version,
            if receipt.changed { "" } else { " (unchanged)" }
        );
    }
    Ok(())
}

fn render_engine_deactivate(engine_id: &str, json: bool) -> Result<(), YuError> {
    let manager = EngineManager::discover().map_err(map_manager_error)?;
    let descriptor = managed_descriptor(engine_id);
    let receipt = manager.deactivate(&descriptor).map_err(map_manager_error)?;

    if json {
        print_json(&ResultEnvelope::new("engine.deactivate", receipt));
    } else if receipt.changed {
        println!(
            "Deactivated {} (previous: {})",
            receipt.engine_id,
            receipt.previous_version.as_deref().unwrap_or("-")
        );
    } else {
        println!("{} is already inactive", receipt.engine_id);
    }
    Ok(())
}

fn render_engine_remove(engine_id: &str, version: &str, json: bool) -> Result<(), YuError> {
    let manager = EngineManager::discover().map_err(map_manager_error)?;
    let descriptor = managed_descriptor(engine_id);
    let receipt = manager
        .remove_version(&descriptor, version)
        .map_err(map_manager_error)?;

    if json {
        print_json(&ResultEnvelope::new("engine.remove", receipt));
    } else {
        println!("Removed {} {}", receipt.engine_id, receipt.version);
        if !receipt.cleanup_complete {
            println!(
                "Cleanup pending: {}",
                receipt
                    .cleanup_path
                    .as_ref()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| "-".to_owned())
            );
        }
    }
    Ok(())
}

fn read_manifest(path: &Path) -> Result<EngineManifest, YuError> {
    let bytes = fs::read(path).map_err(|error| {
        YuError::new(
            ErrorCode::InvalidInput,
            format!("cannot read engine manifest {}: {error}", path.display()),
        )
    })?;

    let manifest: EngineManifest = serde_json::from_slice(&bytes).map_err(|error| {
        YuError::new(
            ErrorCode::InvalidInput,
            format!("cannot parse engine manifest {}: {error}", path.display()),
        )
    })?;
    manifest.validate().map_err(map_manager_error)?;
    Ok(manifest)
}

fn managed_descriptor(engine_id: &str) -> EngineDescriptor {
    EngineDescriptor {
        id: engine_id.to_owned(),
        display_name: engine_id.to_owned(),
        provider: EngineProvider::Managed,
        state: EngineState::Ready,
        version: None,
        capabilities: Vec::new(),
    }
}

fn map_manager_error(error: ManagerError) -> YuError {
    let code = match error {
        ManagerError::InvalidManifest(_) | ManagerError::Archive(_) => ErrorCode::InvalidInput,
        ManagerError::Incompatible(_) | ManagerError::Ownership(_) => ErrorCode::EngineIncompatible,
        ManagerError::NotInstalled(_) => ErrorCode::EngineUnavailable,
        ManagerError::AlreadyInstalled(_)
        | ManagerError::ActiveVersion(_)
        | ManagerError::Busy(_) => ErrorCode::OutputConflict,
        ManagerError::Integrity(_) => ErrorCode::VerificationFailed,
        ManagerError::Download(_)
        | ManagerError::Probe(_)
        | ManagerError::State(_)
        | ManagerError::Environment(_)
        | ManagerError::Io(_) => ErrorCode::ExecutionFailed,
    };

    YuError::new(code, error.to_string())
}

fn render_image_info(
    registry: &RuntimeRegistry,
    file: PathBuf,
    requested_engine: Option<&str>,
    json: bool,
) -> Result<(), YuError> {
    let descriptor = registry.resolve_engine("image.info", requested_engine)?;
    let engine = image_engine(descriptor)?;
    let result = engine.info(&file).map_err(map_image_error)?;

    if json {
        print_json(&ResultEnvelope::new("image.info", result).with_engine(descriptor));
        return Ok(());
    }

    println!("Path: {}", result.path);
    println!("Format: {}", result.format);
    println!("Size: {}x{}", result.width, result.height);
    println!("Color: {}", result.color_type);
    println!("Bit depth: {}", result.bit_depth);
    println!("Channels: {}", result.channels);
    println!("Alpha: {}", result.has_alpha);
    println!("Engine: {}", descriptor.id);
    Ok(())
}

fn render_image_resize(
    registry: &RuntimeRegistry,
    request: ResizeRequest,
    requested_engine: Option<&str>,
    json: bool,
) -> Result<(), YuError> {
    let descriptor = registry.resolve_engine("image.resize", requested_engine)?;
    let engine = image_engine(descriptor)?;
    let result = engine.resize(&request).map_err(map_image_error)?;

    if json {
        print_json(&ResultEnvelope::new("image.resize", result).with_engine(descriptor));
        return Ok(());
    }

    println!(
        "{} {} -> {} ({}x{} -> {}x{})",
        if result.dry_run {
            "Would resize"
        } else {
            "Resized"
        },
        result.input,
        result.output,
        result.source_width,
        result.source_height,
        result.width,
        result.height
    );
    println!("Format: {}", result.format);
    println!("Engine: {}", descriptor.id);
    if result.dry_run {
        println!("Dry run: no output file written");
    }
    Ok(())
}

fn render_image_crop(
    registry: &RuntimeRegistry,
    request: CropRequest,
    requested_engine: Option<&str>,
    json: bool,
) -> Result<(), YuError> {
    let descriptor = registry.resolve_engine("image.crop", requested_engine)?;
    let result = image_engine(descriptor)?
        .crop(&request)
        .map_err(map_image_error)?;
    if json {
        print_json(&ResultEnvelope::new("image.crop", result).with_engine(descriptor));
    } else {
        println!(
            "{} {} -> {} (x={}, y={}, {}x{})",
            if result.dry_run {
                "Would crop"
            } else {
                "Cropped"
            },
            result.input,
            result.output,
            result.x,
            result.y,
            result.width,
            result.height
        );
        println!("Format: {}", result.format);
        println!("Engine: {}", descriptor.id);
        if result.dry_run {
            println!("Dry run: no output file written");
        }
    }
    Ok(())
}

fn render_image_rotate(
    registry: &RuntimeRegistry,
    request: RotateRequest,
    requested_engine: Option<&str>,
    json: bool,
) -> Result<(), YuError> {
    let descriptor = registry.resolve_engine("image.rotate", requested_engine)?;
    let result = image_engine(descriptor)?
        .rotate(&request)
        .map_err(map_image_error)?;
    if json {
        print_json(&ResultEnvelope::new("image.rotate", result).with_engine(descriptor));
    } else {
        println!(
            "{} {} -> {} ({} degrees clockwise, {}x{})",
            if result.dry_run {
                "Would rotate"
            } else {
                "Rotated"
            },
            result.input,
            result.output,
            result.degrees,
            result.width,
            result.height
        );
        println!("Format: {}", result.format);
        println!("Engine: {}", descriptor.id);
        if result.dry_run {
            println!("Dry run: no output file written");
        }
    }
    Ok(())
}

fn render_image_convert(
    registry: &RuntimeRegistry,
    request: ConvertRequest,
    requested_engine: Option<&str>,
    json: bool,
) -> Result<(), YuError> {
    let descriptor = registry.resolve_engine("image.convert", requested_engine)?;
    let result = image_engine(descriptor)?
        .convert(&request)
        .map_err(map_image_error)?;
    if json {
        print_json(&ResultEnvelope::new("image.convert", result).with_engine(descriptor));
    } else {
        println!(
            "{} {} -> {} ({} -> {}, {}x{})",
            if result.dry_run {
                "Would convert"
            } else {
                "Converted"
            },
            result.input,
            result.output,
            result.source_format,
            result.format,
            result.width,
            result.height
        );
        println!("Engine: {}", descriptor.id);
        if result.dry_run {
            println!("Dry run: no output file written");
        }
    }
    Ok(())
}

fn image_engine(descriptor: &EngineDescriptor) -> Result<RustImageEngine, YuError> {
    if descriptor.id == RASTER_ENGINE_ID {
        return Ok(RustImageEngine);
    }

    Err(YuError::new(
        ErrorCode::EngineUnavailable,
        format!(
            "image engine {} is not wired into this build",
            descriptor.id
        ),
    ))
}

fn map_image_error(error: ImageOperationError) -> YuError {
    let code = match error.kind {
        ImageErrorKind::InvalidInput => ErrorCode::InvalidInput,
        ImageErrorKind::Unsupported => ErrorCode::UnsupportedCapability,
        ImageErrorKind::OutputConflict => ErrorCode::OutputConflict,
        ImageErrorKind::Execution => ErrorCode::ExecutionFailed,
    };

    YuError::new(code, error.message)
}

fn render_error(error: &YuError, json: bool) {
    if json {
        print_json_to_stderr(&ErrorEnvelope::from(error));
    } else {
        eprintln!("error {error}");
    }
}

fn print_json<T: Serialize>(value: &T) {
    let rendered =
        serde_json::to_string_pretty(value).expect("serializing YuTool output should not fail");
    println!("{rendered}");
}

fn print_json_to_stderr<T: Serialize>(value: &T) {
    let rendered =
        serde_json::to_string_pretty(value).expect("serializing YuTool error should not fail");
    eprintln!("{rendered}");
}
