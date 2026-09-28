use clap::{Args, Subcommand};
use std::{path::PathBuf, time::Duration};
use yu_capability_psd::{PsdLayerNode, PsdLayerSummary};
use yu_core::{ErrorCode, YuError};
use yu_engine_manager::EngineManager;
use yu_runtime_psd::{DEFAULT_TIMEOUT_SECS, PsdReadResult, ReadOperation};

#[derive(Debug, Subcommand)]
pub(super) enum PsdCommand {
    /// Inspect document metadata without changing the file.
    Inspect(PsdInput),
    /// Read the canonical logical layer tree.
    Tree(PsdInput),
    /// Inspect layers by stable ID.
    Layer {
        #[command(subcommand)]
        command: LayerCommand,
    },
}

#[derive(Debug, Args)]
pub(super) struct PsdInput {
    file: PathBuf,
    #[arg(long)]
    engine: Option<String>,
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECS, value_parser = clap::value_parser!(u64).range(1..=3600))]
    timeout_secs: u64,
}

#[derive(Debug, Subcommand)]
pub(super) enum LayerCommand {
    /// Export one stored 8-bit RGB layer bitmap to a new RGBA8 PNG.
    Export {
        #[command(flatten)]
        input: PsdInput,
        #[arg(long, value_name = "LAYER_ID")]
        id: String,
        #[arg(short, long)]
        output: PathBuf,
    },
    /// List canonical layers, including groups.
    List(PsdInput),
    /// Inspect one layer by its canonical ID.
    Info {
        #[command(flatten)]
        input: PsdInput,
        #[arg(long, value_name = "LAYER_ID")]
        id: String,
    },
}

pub(super) fn run(command: PsdCommand, json: bool) -> Result<(), YuError> {
    let (input, operation) = match command {
        PsdCommand::Layer {
            command: LayerCommand::Export { input, id, output },
        } => {
            let id = id
                .parse()
                .map_err(|e: yu_capability_psd::PsdContractError| {
                    YuError::new(ErrorCode::InvalidArgument, e.to_string())
                })?;
            let manager = EngineManager::discover().map_err(super::map_manager_error)?;
            let envelope = yu_runtime_psd::export_layer(
                &manager,
                &input.file,
                id,
                &output,
                input.engine.as_deref(),
                Duration::from_secs(input.timeout_secs),
            )?;
            if json {
                super::print_json(&envelope);
            } else {
                println!(
                    "Exported {} -> {} ({}x{}, RGBA8 PNG)",
                    envelope.result.layer_id,
                    envelope.result.output_path,
                    envelope.result.width,
                    envelope.result.height
                );
                if let Some(engine) = &envelope.engine {
                    println!(
                        "Engine: {} [{}] {}",
                        engine.id,
                        engine.provider,
                        engine.version.as_deref().unwrap_or("-")
                    );
                }
                for warning in &envelope.warnings {
                    eprintln!("warning: {}", warning.escape_debug());
                }
            }
            return Ok(());
        }
        PsdCommand::Inspect(input) => (input, ReadOperation::Inspect),
        PsdCommand::Tree(input) => (input, ReadOperation::Tree),
        PsdCommand::Layer {
            command: LayerCommand::List(input),
        } => (input, ReadOperation::LayerList),
        PsdCommand::Layer {
            command: LayerCommand::Info { input, id },
        } => {
            let id = id
                .parse()
                .map_err(|e: yu_capability_psd::PsdContractError| {
                    YuError::new(ErrorCode::InvalidArgument, e.to_string())
                })?;
            (input, ReadOperation::LayerInfo(id))
        }
    };
    let manager = EngineManager::discover().map_err(super::map_manager_error)?;
    let envelope = yu_runtime_psd::execute(
        &manager,
        operation,
        &input.file,
        input.engine.as_deref(),
        Duration::from_secs(input.timeout_secs),
    )?;
    if json {
        super::print_json(&envelope);
        return Ok(());
    }
    let document = envelope.result.document();
    println!("Format: {:?}", document.format);
    println!("Size: {}x{}", document.width, document.height);
    println!(
        "Color: {:?}, {} bits/channel, {} channels",
        document.color_mode, document.bits_per_channel, document.channels
    );
    println!(
        "Layers: {} (maximum depth: {})",
        document.layer_count, document.maximum_tree_depth
    );
    if let Some(engine) = &envelope.engine {
        println!(
            "Engine: {} [{}] {}",
            engine.id,
            engine.provider,
            engine.version.as_deref().unwrap_or("-")
        );
    }
    match &envelope.result {
        PsdReadResult::Inspect(_) => {}
        PsdReadResult::Tree(result) => render_tree(&result.layers),
        PsdReadResult::LayerList(result) => {
            for layer in &result.layers {
                render_layer(layer, false);
            }
        }
        PsdReadResult::LayerInfo(result) => {
            render_layer(&result.layer, false);
            println!(
                "Parent: {}",
                result
                    .layer
                    .parent_id
                    .as_ref()
                    .map(|id| id.as_str())
                    .unwrap_or("-")
            );
            println!("Depth: {}", result.layer.depth);
            println!("Children: {}", result.layer.child_count);
            println!("Pixel mask: {}", result.layer.has_pixel_mask);
            println!("Vector mask: {}", result.layer.has_vector_mask);
            if let Some(bounds) = result.layer.bounds {
                println!(
                    "Bounds: top={} left={} bottom={} right={}",
                    bounds.top, bounds.left, bounds.bottom, bounds.right
                );
            }
        }
    }
    for warning in &envelope.warnings {
        eprintln!("warning: {}", warning.escape_debug());
    }
    Ok(())
}

fn render_tree(layers: &[PsdLayerNode]) {
    let mut stack: Vec<_> = layers.iter().rev().collect();
    while let Some(node) = stack.pop() {
        render_layer(&node.layer, true);
        stack.extend(node.children.iter().rev());
    }
}

fn render_layer(layer: &PsdLayerSummary, indent: bool) {
    let padding = if indent {
        "  ".repeat(layer.depth.saturating_sub(1).min(64))
    } else {
        String::new()
    };
    println!(
        "{padding}{} [{:?}] {} ({})",
        layer.id,
        layer.kind,
        layer.name.escape_debug(),
        if layer.visible { "visible" } else { "hidden" }
    );
}
