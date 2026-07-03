use serde::Serialize;
use serde_json::Value;

use crate::error::{AppError, ErrorCode};

pub const ENVELOPE_VERSION: &str = "2.1";

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
        ErrorCode::AppUnresponsive => Some("wait;execute_by_ref".to_owned()),
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
#[path = "output_tests.rs"]
mod tests;
