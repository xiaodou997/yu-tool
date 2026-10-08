use serde::Serialize;
use std::{error::Error, fmt};

pub use yu_engine_api::{EngineDescriptor, EngineProvider, EngineState};

pub const SCHEMA_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineRef {
    pub id: String,
    pub provider: EngineProvider,
    pub version: Option<String>,
}

impl From<&EngineDescriptor> for EngineRef {
    fn from(engine: &EngineDescriptor) -> Self {
        Self {
            id: engine.id.clone(),
            provider: engine.provider,
            version: engine.version.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResultEnvelope<T> {
    pub schema_version: &'static str,
    pub operation: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineRef>,
    pub result: T,
    pub warnings: Vec<String>,
}

impl<T> ResultEnvelope<T> {
    pub fn new(operation: &'static str, result: T) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            operation,
            engine: None,
            result,
            warnings: Vec::new(),
        }
    }

    pub fn with_engine(mut self, engine: &EngineDescriptor) -> Self {
        self.engine = Some(engine.into());
        self
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        self.warnings.push(warning.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    InvalidArgument,
    InvalidInput,
    UnsupportedCapability,
    EngineUnavailable,
    EngineIncompatible,
    ExecutionFailed,
    OutputConflict,
    VerificationFailed,
}

impl ErrorCode {
    pub const fn exit_code(self) -> u8 {
        match self {
            Self::ExecutionFailed | Self::VerificationFailed => 1,
            Self::InvalidArgument | Self::InvalidInput | Self::OutputConflict => 2,
            Self::UnsupportedCapability | Self::EngineUnavailable | Self::EngineIncompatible => 3,
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::InvalidArgument => "INVALID_ARGUMENT",
            Self::InvalidInput => "INVALID_INPUT",
            Self::UnsupportedCapability => "UNSUPPORTED_CAPABILITY",
            Self::EngineUnavailable => "ENGINE_UNAVAILABLE",
            Self::EngineIncompatible => "ENGINE_INCOMPATIBLE",
            Self::ExecutionFailed => "EXECUTION_FAILED",
            Self::OutputConflict => "OUTPUT_CONFLICT",
            Self::VerificationFailed => "VERIFICATION_FAILED",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct YuError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<EngineRef>,
}

impl YuError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            engine: None,
        }
    }

    pub fn with_engine(mut self, engine: &EngineDescriptor) -> Self {
        self.engine = Some(engine.into());
        self
    }

    pub const fn exit_code(&self) -> u8 {
        self.code.exit_code()
    }
}

impl fmt::Display for YuError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl Error for YuError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorEnvelope<'a> {
    pub schema_version: &'static str,
    pub error: ErrorBody<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<&'a EngineRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorBody<'a> {
    pub code: ErrorCode,
    pub message: &'a str,
}

impl<'a> From<&'a YuError> for ErrorEnvelope<'a> {
    fn from(error: &'a YuError) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            error: ErrorBody {
                code: error.code,
                message: &error.message,
            },
            engine: error.engine.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CapabilityDescriptor {
    pub id: String,
    pub summary: String,
    pub engines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformInfo {
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EngineSummary {
    pub total: usize,
    pub ready: usize,
    pub built_in: usize,
    pub managed: usize,
    pub system: usize,
    pub unhealthy: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DoctorReport {
    pub version: String,
    pub healthy: bool,
    pub platform: PlatformInfo,
    pub capabilities: usize,
    pub engines: EngineSummary,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveErrorKind {
    UnknownEngine,
    EngineUnavailable,
    EngineIncompatible,
    NoCompatibleEngine,
}

#[derive(Debug, Clone)]
pub struct ResolveError {
    pub kind: ResolveErrorKind,
    pub message: String,
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ResolveError {}

impl From<ResolveError> for YuError {
    fn from(error: ResolveError) -> Self {
        let code = match error.kind {
            ResolveErrorKind::UnknownEngine | ResolveErrorKind::EngineUnavailable => {
                ErrorCode::EngineUnavailable
            }
            ResolveErrorKind::EngineIncompatible => ErrorCode::EngineIncompatible,
            ResolveErrorKind::NoCompatibleEngine => ErrorCode::UnsupportedCapability,
        };

        Self::new(code, error.message)
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeRegistry {
    capabilities: Vec<CapabilityDescriptor>,
    engines: Vec<EngineDescriptor>,
}

impl RuntimeRegistry {
    pub fn bootstrap() -> Self {
        let capabilities = vec![
            CapabilityDescriptor {
                id: "runtime.doctor".to_owned(),
                summary: "Diagnose YuTool runtime and engine health.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "runtime.capabilities".to_owned(),
                summary: "List capabilities available on this machine.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "engine.list".to_owned(),
                summary: "List known engines and their current state.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "engine.info".to_owned(),
                summary: "Inspect discovered engine providers and versions.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "engine.install".to_owned(),
                summary: "Install a managed engine from a manifest.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "engine.versions".to_owned(),
                summary: "List installed managed-engine versions.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "engine.activate".to_owned(),
                summary: "Activate an installed managed-engine version.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "engine.deactivate".to_owned(),
                summary: "Deactivate a managed engine.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "engine.remove".to_owned(),
                summary: "Remove an inactive managed-engine version.".to_owned(),
                engines: vec!["yu-runtime".to_owned()],
            },
            CapabilityDescriptor {
                id: "image.info".to_owned(),
                summary: "Inspect a raster image.".to_owned(),
                engines: vec!["raster-rs".to_owned()],
            },
            CapabilityDescriptor {
                id: "image.resize".to_owned(),
                summary: "Resize a raster image.".to_owned(),
                engines: vec!["raster-rs".to_owned()],
            },
            CapabilityDescriptor {
                id: "image.crop".to_owned(),
                summary: "Crop a raster image by a bounded rectangle.".to_owned(),
                engines: vec!["raster-rs".to_owned()],
            },
            CapabilityDescriptor {
                id: "image.rotate".to_owned(),
                summary: "Rotate a raster image clockwise by 90, 180, or 270 degrees.".to_owned(),
                engines: vec!["raster-rs".to_owned()],
            },
            CapabilityDescriptor {
                id: "image.convert".to_owned(),
                summary: "Convert raster output to PNG, JPEG, or WebP.".to_owned(),
                engines: vec!["raster-rs".to_owned()],
            },
        ];

        let engines = vec![
            EngineDescriptor {
                id: "yu-runtime".to_owned(),
                display_name: "YuTool Runtime".to_owned(),
                provider: EngineProvider::BuiltIn,
                state: EngineState::Ready,
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                capabilities: vec![
                    "runtime.doctor".to_owned(),
                    "runtime.capabilities".to_owned(),
                    "engine.list".to_owned(),
                    "engine.info".to_owned(),
                    "engine.install".to_owned(),
                    "engine.versions".to_owned(),
                    "engine.activate".to_owned(),
                    "engine.deactivate".to_owned(),
                    "engine.remove".to_owned(),
                ],
            },
            EngineDescriptor {
                id: "raster-rs".to_owned(),
                display_name: "YuTool Rust Raster Engine".to_owned(),
                provider: EngineProvider::BuiltIn,
                state: EngineState::Ready,
                version: Some(env!("CARGO_PKG_VERSION").to_owned()),
                capabilities: vec![
                    "image.info".to_owned(),
                    "image.resize".to_owned(),
                    "image.crop".to_owned(),
                    "image.rotate".to_owned(),
                    "image.convert".to_owned(),
                ],
            },
        ];

        Self {
            capabilities,
            engines,
        }
    }

    pub fn capabilities(&self) -> &[CapabilityDescriptor] {
        &self.capabilities
    }

    pub fn engines(&self) -> &[EngineDescriptor] {
        &self.engines
    }

    pub fn resolve_engine(
        &self,
        capability: &str,
        requested_engine: Option<&str>,
    ) -> Result<&EngineDescriptor, ResolveError> {
        if let Some(requested_engine) = requested_engine {
            let engine = self
                .engines
                .iter()
                .find(|engine| engine.id == requested_engine)
                .ok_or_else(|| ResolveError {
                    kind: ResolveErrorKind::UnknownEngine,
                    message: format!("unknown engine: {requested_engine}"),
                })?;

            if engine.state != EngineState::Ready {
                return Err(ResolveError {
                    kind: ResolveErrorKind::EngineUnavailable,
                    message: format!(
                        "engine {} is not ready (state: {})",
                        engine.id, engine.state
                    ),
                });
            }

            if !engine.capabilities.iter().any(|item| item == capability) {
                return Err(ResolveError {
                    kind: ResolveErrorKind::EngineIncompatible,
                    message: format!(
                        "engine {} does not provide capability {capability}",
                        engine.id
                    ),
                });
            }

            return Ok(engine);
        }

        self.engines
            .iter()
            .filter(|engine| {
                engine.state == EngineState::Ready
                    && engine.capabilities.iter().any(|item| item == capability)
            })
            .min_by_key(|engine| provider_priority(engine.provider))
            .ok_or_else(|| ResolveError {
                kind: ResolveErrorKind::NoCompatibleEngine,
                message: format!("no ready engine provides capability {capability}"),
            })
    }

    pub fn doctor_report(&self) -> DoctorReport {
        self.doctor_report_with_engines(&self.engines)
    }

    pub fn doctor_report_with_engines(&self, engines: &[EngineDescriptor]) -> DoctorReport {
        let ready = engines
            .iter()
            .filter(|engine| engine.state == EngineState::Ready)
            .count();
        let built_in = engines
            .iter()
            .filter(|engine| engine.provider == EngineProvider::BuiltIn)
            .count();
        let managed = engines
            .iter()
            .filter(|engine| engine.provider == EngineProvider::Managed)
            .count();
        let system = engines
            .iter()
            .filter(|engine| engine.provider == EngineProvider::System)
            .count();
        let unhealthy = engines
            .iter()
            .filter(|engine| engine_is_unhealthy(engine))
            .count();

        DoctorReport {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            healthy: unhealthy == 0,
            platform: PlatformInfo {
                os: std::env::consts::OS.to_owned(),
                arch: std::env::consts::ARCH.to_owned(),
            },
            capabilities: self.capabilities.len(),
            engines: EngineSummary {
                total: engines.len(),
                ready,
                built_in,
                managed,
                system,
                unhealthy,
            },
        }
    }
}

impl Default for RuntimeRegistry {
    fn default() -> Self {
        Self::bootstrap()
    }
}

fn engine_is_unhealthy(engine: &EngineDescriptor) -> bool {
    match engine.state {
        EngineState::Ready | EngineState::Disabled => false,
        EngineState::NotInstalled => engine.provider != EngineProvider::System,
        EngineState::Broken | EngineState::Incompatible => true,
    }
}

fn provider_priority(provider: EngineProvider) -> u8 {
    match provider {
        EngineProvider::BuiltIn => 0,
        EngineProvider::Managed => 1,
        EngineProvider::System => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_registry_is_healthy() {
        let registry = RuntimeRegistry::bootstrap();
        let report = registry.doctor_report();

        assert!(report.healthy);
        assert_eq!(report.engines.total, 2);
        assert_eq!(report.engines.ready, 2);
        assert_eq!(report.engines.built_in, 2);
        assert_eq!(report.engines.managed, 0);
        assert_eq!(report.engines.system, 0);
        assert_eq!(report.engines.unhealthy, 0);
    }

    #[test]
    fn bootstrap_registry_exposes_runtime_and_image_capabilities() {
        let registry = RuntimeRegistry::bootstrap();
        let ids: Vec<_> = registry
            .capabilities()
            .iter()
            .map(|capability| capability.id.as_str())
            .collect();

        assert_eq!(
            ids,
            vec![
                "runtime.doctor",
                "runtime.capabilities",
                "engine.list",
                "engine.info",
                "engine.install",
                "engine.versions",
                "engine.activate",
                "engine.deactivate",
                "engine.remove",
                "image.info",
                "image.resize",
                "image.crop",
                "image.rotate",
                "image.convert"
            ]
        );
    }

    #[test]
    fn automatic_resolution_selects_built_in_raster_engine() {
        let registry = RuntimeRegistry::bootstrap();
        let engine = registry.resolve_engine("image.info", None).unwrap();

        assert_eq!(engine.id, "raster-rs");
        assert_eq!(engine.provider, EngineProvider::BuiltIn);
    }

    #[test]
    fn explicit_incompatible_engine_is_rejected() {
        let registry = RuntimeRegistry::bootstrap();
        let error = registry
            .resolve_engine("image.info", Some("yu-runtime"))
            .unwrap_err();

        assert_eq!(error.kind, ResolveErrorKind::EngineIncompatible);
    }

    #[test]
    fn unknown_engine_is_rejected() {
        let registry = RuntimeRegistry::bootstrap();
        let error = registry
            .resolve_engine("image.info", Some("does-not-exist"))
            .unwrap_err();

        assert_eq!(error.kind, ResolveErrorKind::UnknownEngine);
    }

    #[test]
    fn result_envelope_serializes_stable_schema() {
        let registry = RuntimeRegistry::bootstrap();
        let engine = registry.resolve_engine("image.info", None).unwrap();
        let envelope = ResultEnvelope::new("image.info", "ok").with_engine(engine);
        let json = serde_json::to_value(envelope).unwrap();

        assert_eq!(json["schema_version"], "1");
        assert_eq!(json["operation"], "image.info");
        assert_eq!(json["engine"]["id"], "raster-rs");
        assert_eq!(json["engine"]["provider"], "built_in");
        assert_eq!(json["result"], "ok");
    }

    #[test]
    fn error_envelope_serializes_stable_error_code() {
        let error = YuError::new(ErrorCode::OutputConflict, "already exists");
        let json = serde_json::to_value(ErrorEnvelope::from(&error)).unwrap();

        assert_eq!(json["schema_version"], "1");
        assert_eq!(json["error"]["code"], "OUTPUT_CONFLICT");
        assert_eq!(json["error"]["message"], "already exists");
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn resolve_errors_map_to_protocol_errors() {
        let registry = RuntimeRegistry::bootstrap();
        let resolve = registry
            .resolve_engine("image.info", Some("yu-runtime"))
            .unwrap_err();
        let error = YuError::from(resolve);

        assert_eq!(error.code, ErrorCode::EngineIncompatible);
        assert_eq!(error.exit_code(), 3);
    }
}
