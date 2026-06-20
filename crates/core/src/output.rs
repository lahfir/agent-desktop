use serde::Serialize;
use serde_json::Value;

use crate::error::AppError;

pub const ENVELOPE_VERSION: &str = "2.0";

/// Structured output envelope used by the CLI and future programmatic transports.
#[derive(Debug, Serialize)]
pub struct Response {
    pub version: &'static str,
    pub ok: bool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<AppContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

#[derive(Debug, Serialize)]
pub struct AppContext {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowContext>,
}

#[derive(Debug, Serialize)]
pub struct WindowContext {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}

impl Response {
    pub fn ok(command: impl Into<String>, data: Value) -> Self {
        Self {
            version: ENVELOPE_VERSION,
            ok: true,
            command: command.into(),
            app: None,
            data: Some(data),
            error: None,
        }
    }

    pub fn err(command: impl Into<String>, payload: ErrorPayload) -> Self {
        Self {
            version: ENVELOPE_VERSION,
            ok: false,
            command: command.into(),
            app: None,
            data: None,
            error: Some(payload),
        }
    }

    pub fn with_app(mut self, ctx: AppContext) -> Self {
        self.app = Some(ctx);
        self
    }
}

impl ErrorPayload {
    pub fn from_app_error(err: &AppError) -> Self {
        let mut payload = Self::new(err.code(), err.to_string());
        if let Some(suggestion) = err.suggestion() {
            payload = payload.with_suggestion(suggestion);
        }
        if let AppError::Adapter(adapter_error) = err {
            payload.platform_detail = adapter_error.platform_detail.clone();
            payload.details = adapter_error.details.clone();
        }
        payload
    }

    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            suggestion: None,
            retry_command: None,
            platform_detail: None,
            details: None,
        }
    }

    pub fn with_suggestion(mut self, s: impl Into<String>) -> Self {
        self.suggestion = Some(s.into());
        self
    }

    pub fn with_retry(mut self, cmd: impl Into<String>) -> Self {
        self.retry_command = Some(cmd.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::{AdapterError, ErrorCode};
    use serde_json::json;

    #[test]
    fn app_error_payload_preserves_adapter_recovery_fields() {
        let err = AppError::Adapter(
            AdapterError::new(ErrorCode::ActionFailed, "not actionable")
                .with_suggestion("wait and retry")
                .with_platform_detail("native press action failed")
                .with_details(json!({ "check": "visible" })),
        );

        let payload = ErrorPayload::from_app_error(&err);

        assert_eq!(payload.code, "ACTION_FAILED");
        assert_eq!(payload.message, "not actionable");
        assert_eq!(payload.suggestion.as_deref(), Some("wait and retry"));
        assert_eq!(
            payload.platform_detail.as_deref(),
            Some("native press action failed")
        );
        assert_eq!(payload.details, Some(json!({ "check": "visible" })));
    }
}
