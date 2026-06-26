use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, ErrorCode};

pub const ENVELOPE_VERSION: &str = "2.0";

/// Structured output envelope used by the CLI and future programmatic transports.
#[derive(Debug, Serialize)]
pub struct Response {
    pub version: &'static str,
    pub ok: bool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
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
            data: Some(data),
            error: None,
        }
    }

    pub fn err(command: impl Into<String>, payload: ErrorPayload) -> Self {
        Self {
            version: ENVELOPE_VERSION,
            ok: false,
            command: command.into(),
            data: None,
            error: Some(payload),
        }
    }
}

fn retry_token_for_code(code: &ErrorCode) -> Option<String> {
    match code {
        ErrorCode::StaleRef | ErrorCode::SnapshotNotFound => {
            Some("snapshot;execute_by_ref".to_owned())
        }
        ErrorCode::PolicyDenied => Some("escalate_policy".to_owned()),
        _ => None,
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
            payload.retry_command = retry_token_for_code(&adapter_error.code);
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
        assert_eq!(
            payload.retry_command, None,
            "ACTION_FAILED must not carry a retry token"
        );
    }

    #[test]
    fn stale_ref_payload_carries_snapshot_retry_token() {
        let err = AppError::stale_ref("@e5");
        let payload = ErrorPayload::from_app_error(&err);
        assert_eq!(payload.code, "STALE_REF");
        assert_eq!(
            payload.retry_command.as_deref(),
            Some("snapshot;execute_by_ref"),
            "STALE_REF must carry the canonical retry token"
        );
    }

    #[test]
    fn snapshot_not_found_payload_carries_snapshot_retry_token() {
        let err = AppError::Adapter(AdapterError::snapshot_not_found("snap-abc"));
        let payload = ErrorPayload::from_app_error(&err);
        assert_eq!(payload.code, "SNAPSHOT_NOT_FOUND");
        assert_eq!(
            payload.retry_command.as_deref(),
            Some("snapshot;execute_by_ref"),
            "SNAPSHOT_NOT_FOUND must carry the canonical retry token"
        );
    }

    #[test]
    fn policy_denied_payload_carries_escalate_policy_token() {
        let err = AppError::Adapter(AdapterError::policy_denied("blocked by policy"));
        let payload = ErrorPayload::from_app_error(&err);
        assert_eq!(payload.code, "POLICY_DENIED");
        assert_eq!(
            payload.retry_command.as_deref(),
            Some("escalate_policy"),
            "POLICY_DENIED must carry the escalate_policy token, not a snapshot token"
        );
    }

    #[test]
    fn retry_command_absent_for_non_retryable_errors() {
        for err in [
            AppError::Adapter(AdapterError::new(ErrorCode::InvalidArgs, "bad input")),
            AppError::Adapter(AdapterError::not_supported("method_x")),
            AppError::Adapter(AdapterError::new(ErrorCode::ActionFailed, "failed")),
        ] {
            let payload = ErrorPayload::from_app_error(&err);
            assert!(
                payload.retry_command.is_none(),
                "non-retryable error {} must not carry a retry token",
                payload.code
            );
        }
    }
}
