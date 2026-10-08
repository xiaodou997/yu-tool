use serde::Serialize;
use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageErrorKind {
    InvalidInput,
    Unsupported,
    OutputConflict,
    Execution,
}

#[derive(Debug, Clone)]
pub struct ImageOperationError {
    pub kind: ImageErrorKind,
    pub message: String,
}

impl ImageOperationError {
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self {
            kind: ImageErrorKind::InvalidInput,
            message: message.into(),
        }
    }

    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            kind: ImageErrorKind::Unsupported,
            message: message.into(),
        }
    }

    pub fn output_conflict(message: impl Into<String>) -> Self {
        Self {
            kind: ImageErrorKind::OutputConflict,
            message: message.into(),
        }
    }

    pub fn execution(message: impl Into<String>) -> Self {
        Self {
            kind: ImageErrorKind::Execution,
            message: message.into(),
        }
    }
}

impl fmt::Display for ImageOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for ImageOperationError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ImageInfo {
    pub path: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub color_type: String,
    pub bit_depth: u8,
    pub channels: u8,
    pub has_alpha: bool,
}

/// Absence preserves no-clobber behavior. Presence permits replacement
/// only when the destination's current bytes match this SHA-256.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OutputPolicy {
    pub expected_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResizeRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub dry_run: bool,
    pub output_policy: OutputPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResizeResult {
    pub input: String,
    pub output: String,
    pub source_width: u32,
    pub source_height: u32,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub dry_run: bool,
    pub replaced: bool,
    pub would_replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CropRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub dry_run: bool,
    pub output_policy: OutputPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CropResult {
    pub input: String,
    pub output: String,
    pub source_width: u32,
    pub source_height: u32,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub dry_run: bool,
    pub replaced: bool,
    pub would_replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RotateRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub degrees: u16,
    pub dry_run: bool,
    pub output_policy: OutputPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RotateResult {
    pub input: String,
    pub output: String,
    pub source_width: u32,
    pub source_height: u32,
    pub degrees: u16,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub dry_run: bool,
    pub replaced: bool,
    pub would_replace: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConvertRequest {
    pub input: PathBuf,
    pub output: PathBuf,
    pub dry_run: bool,
    pub output_policy: OutputPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConvertResult {
    pub input: String,
    pub output: String,
    pub source_format: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub dry_run: bool,
    pub replaced: bool,
    pub would_replace: bool,
}

pub trait ImageEngine {
    fn id(&self) -> &'static str;

    fn info(&self, path: &Path) -> Result<ImageInfo, ImageOperationError>;

    fn resize(&self, request: &ResizeRequest) -> Result<ResizeResult, ImageOperationError>;

    fn crop(&self, request: &CropRequest) -> Result<CropResult, ImageOperationError>;

    fn rotate(&self, request: &RotateRequest) -> Result<RotateResult, ImageOperationError>;

    fn convert(&self, request: &ConvertRequest) -> Result<ConvertResult, ImageOperationError>;
}
