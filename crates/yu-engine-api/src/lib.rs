use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineProvider {
    BuiltIn,
    Managed,
    System,
}

impl fmt::Display for EngineProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::BuiltIn => "built_in",
            Self::Managed => "managed",
            Self::System => "system",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EngineState {
    Ready,
    NotInstalled,
    Broken,
    Incompatible,
    Disabled,
}

impl fmt::Display for EngineState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Ready => "ready",
            Self::NotInstalled => "not_installed",
            Self::Broken => "broken",
            Self::Incompatible => "incompatible",
            Self::Disabled => "disabled",
        };
        f.write_str(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EngineDescriptor {
    pub id: String,
    pub display_name: String,
    pub provider: EngineProvider,
    pub state: EngineState,
    pub version: Option<String>,
    pub capabilities: Vec<String>,
}


pub const EXTERNAL_ENGINE_PROTOCOL_VERSION: &str = "1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalEngineRequest<T> {
    pub protocol_version: String,
    pub request_id: String,
    pub capability: String,
    pub payload: T,
}

impl<T> ExternalEngineRequest<T> {
    pub fn new(
        request_id: impl Into<String>,
        capability: impl Into<String>,
        payload: T,
    ) -> Self {
        Self {
            protocol_version: EXTERNAL_ENGINE_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.into(),
            capability: capability.into(),
            payload,
        }
    }

    pub fn validate_header(&self) -> Result<(), ExternalEngineProtocolError> {
        validate_protocol_version(&self.protocol_version)?;
        validate_request_id(&self.request_id)?;
        validate_capability_id(&self.capability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExternalEngineErrorCode {
    InvalidArgument,
    InvalidInput,
    UnsupportedCapability,
    EngineIncompatible,
    ExecutionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalEngineErrorBody {
    pub code: ExternalEngineErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ExternalEngineResponse<T> {
    Ok {
        protocol_version: String,
        request_id: String,
        result: T,
        #[serde(default)]
        warnings: Vec<String>,
    },
    Error {
        protocol_version: String,
        request_id: String,
        error: ExternalEngineErrorBody,
        #[serde(default)]
        warnings: Vec<String>,
    },
}

impl<T> ExternalEngineResponse<T> {
    pub fn success(request_id: impl Into<String>, result: T) -> Self {
        Self::Ok {
            protocol_version: EXTERNAL_ENGINE_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.into(),
            result,
            warnings: Vec::new(),
        }
    }

    pub fn failure(
        request_id: impl Into<String>,
        code: ExternalEngineErrorCode,
        message: impl Into<String>,
    ) -> Self {
        Self::Error {
            protocol_version: EXTERNAL_ENGINE_PROTOCOL_VERSION.to_owned(),
            request_id: request_id.into(),
            error: ExternalEngineErrorBody {
                code,
                message: message.into(),
            },
            warnings: Vec::new(),
        }
    }

    pub fn protocol_version(&self) -> &str {
        match self {
            Self::Ok {
                protocol_version, ..
            }
            | Self::Error {
                protocol_version, ..
            } => protocol_version,
        }
    }

    pub fn request_id(&self) -> &str {
        match self {
            Self::Ok { request_id, .. } | Self::Error { request_id, .. } => request_id,
        }
    }

    pub fn warnings(&self) -> &[String] {
        match self {
            Self::Ok { warnings, .. } | Self::Error { warnings, .. } => warnings,
        }
    }

    pub fn with_warning(mut self, warning: impl Into<String>) -> Self {
        match &mut self {
            Self::Ok { warnings, .. } | Self::Error { warnings, .. } => {
                warnings.push(warning.into())
            }
        }
        self
    }
}

pub fn validate_external_response<T, U>(
    request: &ExternalEngineRequest<T>,
    response: &ExternalEngineResponse<U>,
) -> Result<(), ExternalEngineProtocolError> {
    request.validate_header()?;
    validate_protocol_version(response.protocol_version())?;

    if response.request_id() != request.request_id {
        return Err(ExternalEngineProtocolError::new(format!(
            "external engine response request_id mismatch: expected {}, got {}",
            request.request_id,
            response.request_id()
        )));
    }

    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalEngineProtocolError {
    pub message: String,
}

impl ExternalEngineProtocolError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ExternalEngineProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ExternalEngineProtocolError {}

fn validate_protocol_version(value: &str) -> Result<(), ExternalEngineProtocolError> {
    if value != EXTERNAL_ENGINE_PROTOCOL_VERSION {
        return Err(ExternalEngineProtocolError::new(format!(
            "unsupported external engine protocol version: {value}"
        )));
    }
    Ok(())
}

fn validate_request_id(value: &str) -> Result<(), ExternalEngineProtocolError> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
        })
    {
        return Err(ExternalEngineProtocolError::new(
            "external engine request_id must be 1..=128 safe ASCII characters",
        ));
    }
    Ok(())
}

fn validate_capability_id(value: &str) -> Result<(), ExternalEngineProtocolError> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b'-')
        })
    {
        return Err(ExternalEngineProtocolError::new(
            "external engine capability must be 1..=128 lowercase ASCII capability characters",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod protocol_tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    struct FixturePayload {
        value: String,
    }

    #[test]
    fn request_serializes_stable_protocol_shape() {
        let request = ExternalEngineRequest::new(
            "req-0001",
            "psd.inspect",
            FixturePayload {
                value: "fixture".to_owned(),
            },
        );
        request.validate_header().unwrap();

        let json = serde_json::to_value(request).unwrap();
        assert_eq!(json["protocol_version"], "1");
        assert_eq!(json["request_id"], "req-0001");
        assert_eq!(json["capability"], "psd.inspect");
        assert_eq!(json["payload"]["value"], "fixture");
    }

    #[test]
    fn success_response_is_correlated_to_request() {
        let request = ExternalEngineRequest::new(
            "req-0002",
            "psd.tree",
            FixturePayload {
                value: "fixture".to_owned(),
            },
        );
        let response = ExternalEngineResponse::success(
            "req-0002",
            FixturePayload {
                value: "ok".to_owned(),
            },
        )
        .with_warning("fixture warning");

        validate_external_response(&request, &response).unwrap();
        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["protocol_version"], "1");
        assert_eq!(json["request_id"], "req-0002");
        assert_eq!(json["result"]["value"], "ok");
        assert_eq!(json["warnings"][0], "fixture warning");
    }

    #[test]
    fn error_response_uses_public_compatible_error_codes() {
        let response = ExternalEngineResponse::<serde_json::Value>::failure(
            "req-0003",
            ExternalEngineErrorCode::UnsupportedCapability,
            "not supported",
        );
        let json = serde_json::to_value(response).unwrap();

        assert_eq!(json["status"], "error");
        assert_eq!(json["error"]["code"], "UNSUPPORTED_CAPABILITY");
        assert_eq!(json["error"]["message"], "not supported");
    }

    #[test]
    fn response_request_id_must_match() {
        let request = ExternalEngineRequest::new("req-a", "psd.inspect", ());
        let response = ExternalEngineResponse::success("req-b", ());
        let error = validate_external_response(&request, &response).unwrap_err();

        assert!(error.message.contains("request_id mismatch"));
    }

    #[test]
    fn protocol_rejects_unsafe_headers() {
        let request = ExternalEngineRequest::new("bad request", "PSD Inspect", ());
        assert!(request.validate_header().is_err());

        let request = ExternalEngineRequest {
            protocol_version: "2".to_owned(),
            request_id: "req-1".to_owned(),
            capability: "psd.inspect".to_owned(),
            payload: (),
        };
        assert!(request.validate_header().is_err());
    }
}
