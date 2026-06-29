use serde::Serialize;
use serde_json::Value;
use thiserror::Error;

use crate::interaction_policy::InteractionPolicy;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    PermDenied,
    ElementNotFound,
    AppNotFound,
    ActionFailed,
    ActionNotSupported,
    StaleRef,
    AmbiguousTarget,
    WindowNotFound,
    PlatformNotSupported,
    Timeout,
    InvalidArgs,
    NotificationNotFound,
    SnapshotNotFound,
    PolicyDenied,
    Internal,
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::PermDenied => "PERM_DENIED",
            ErrorCode::ElementNotFound => "ELEMENT_NOT_FOUND",
            ErrorCode::AppNotFound => "APP_NOT_FOUND",
            ErrorCode::ActionFailed => "ACTION_FAILED",
            ErrorCode::ActionNotSupported => "ACTION_NOT_SUPPORTED",
            ErrorCode::StaleRef => "STALE_REF",
            ErrorCode::AmbiguousTarget => "AMBIGUOUS_TARGET",
            ErrorCode::WindowNotFound => "WINDOW_NOT_FOUND",
            ErrorCode::PlatformNotSupported => "PLATFORM_NOT_SUPPORTED",
            ErrorCode::Timeout => "TIMEOUT",
            ErrorCode::InvalidArgs => "INVALID_ARGS",
            ErrorCode::NotificationNotFound => "NOTIFICATION_NOT_FOUND",
            ErrorCode::SnapshotNotFound => "SNAPSHOT_NOT_FOUND",
            ErrorCode::PolicyDenied => "POLICY_DENIED",
            ErrorCode::Internal => "INTERNAL",
        }
    }
}

#[derive(Debug, Error, Clone)]
#[error("{message}")]
pub struct AdapterError {
    pub code: ErrorCode,
    pub message: String,
    #[source]
    pub source: Option<Box<SourceError>>,
    pub suggestion: Option<String>,
    pub platform_detail: Option<String>,
    pub details: Option<Value>,
}

#[derive(Debug, Error, Clone)]
#[error("{0}")]
pub struct SourceError(pub String);

impl AdapterError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            source: None,
            suggestion: None,
            platform_detail: None,
            details: None,
        }
    }

    pub fn with_suggestion(mut self, s: impl Into<String>) -> Self {
        self.suggestion = Some(s.into());
        self
    }

    pub fn with_platform_detail(mut self, d: impl Into<String>) -> Self {
        self.platform_detail = Some(d.into());
        self
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn stale_ref(ref_id: &str) -> Self {
        Self::new(
            ErrorCode::StaleRef,
            format!("{ref_id} not found in current RefMap"),
        )
        .with_suggestion(
            "Re-run a snapshot to obtain fresh refs, then retry with the new ref \
             (CLI: snapshot [--skeleton]; FFI: ad_snapshot then ad_execute_by_ref with the returned snapshot_id)",
        )
    }

    pub fn ambiguous_target(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::AmbiguousTarget, message).with_suggestion(
            "Re-run a snapshot to refresh refs, then retry with a more specific ref",
        )
    }

    pub fn not_supported(method: &str) -> Self {
        Self::new(
            ErrorCode::PlatformNotSupported,
            format!("{method} is not supported on this platform"),
        )
        .with_suggestion("This platform adapter ships in Phase 2")
    }

    pub fn element_not_found(ref_id: &str) -> Self {
        Self::new(
            ErrorCode::ElementNotFound,
            format!("Element {ref_id} could not be resolved"),
        )
        .with_suggestion("Re-run a snapshot to obtain fresh refs, then retry with the new ref")
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Timeout, msg)
            .with_suggestion("The target application may be busy or unresponsive")
    }

    pub fn notification_not_found(index: usize) -> Self {
        Self::new(
            ErrorCode::NotificationNotFound,
            format!("Notification at index {index} not found"),
        )
        .with_suggestion(
            "Notification may have been dismissed or expired. \
             Re-run a notification list to see current notifications \
             (CLI: list-notifications; FFI: ad_list_notifications)",
        )
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, msg)
    }

    pub fn permission_denied() -> Self {
        Self::new(
            ErrorCode::PermDenied,
            "Accessibility permission not granted",
        )
        .with_suggestion(
            "Open System Settings > Privacy & Security > Accessibility and add the app that launches agent-desktop",
        )
    }

    pub fn snapshot_not_found(snapshot_id: &str) -> Self {
        Self::new(
            ErrorCode::SnapshotNotFound,
            format!("Snapshot '{snapshot_id}' not found"),
        )
        .with_suggestion(
            "Re-run a snapshot and retry with the returned snapshot_id \
             (CLI: snapshot, then pass --snapshot <id>; FFI: ad_snapshot then supply snapshot_id to ad_execute_by_ref)",
        )
    }

    pub fn policy_denied(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PolicyDenied, message).with_suggestion(
            "Use an explicit mouse/focus command if physical interaction is intended",
        )
    }

    pub fn policy_denied_for_policy(message: impl Into<String>, policy: InteractionPolicy) -> Self {
        Self::new(ErrorCode::PolicyDenied, message)
            .with_suggestion(policy_denied_suggestion(policy))
    }
}

fn policy_denied_suggestion(policy: InteractionPolicy) -> &'static str {
    if policy.allow_focus_steal && !policy.allow_cursor_move {
        "Enable cursor movement in the interaction policy to permit cursor-driven actions \
         (CLI: --headed; FFI: set allow_cursor_move in the policy), \
         or use an explicit mouse command if physical input is intended"
    } else if !policy.allow_focus_steal && !policy.allow_cursor_move {
        "Headless mode allows only accessibility-backed actions; \
         enable physical interaction in the policy (CLI: --headed) only if cursor/focus movement is intended, \
         otherwise refresh the snapshot or target an element with the needed semantic action"
    } else {
        "Use an explicit mouse/focus command if physical interaction is intended"
    }
}

#[derive(Debug, Error)]
pub enum AppError {
    #[error(transparent)]
    Adapter(#[from] AdapterError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Internal(String),
}

impl AppError {
    pub fn code(&self) -> &str {
        match self {
            AppError::Adapter(e) => e.code.as_str(),
            AppError::Io(_) | AppError::Json(_) | AppError::Internal(_) => "INTERNAL",
        }
    }

    pub fn suggestion(&self) -> Option<&str> {
        match self {
            AppError::Adapter(e) => e.suggestion.as_deref(),
            _ => None,
        }
    }

    pub fn stale_ref(ref_id: &str) -> Self {
        AppError::Adapter(AdapterError::stale_ref(ref_id))
    }

    pub fn invalid_input(msg: impl Into<String>) -> Self {
        AppError::Adapter(AdapterError::new(ErrorCode::InvalidArgs, msg))
    }

    pub fn invalid_input_with_suggestion(
        msg: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        AppError::Adapter(
            AdapterError::new(ErrorCode::InvalidArgs, msg).with_suggestion(suggestion),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_not_found_error_has_correct_code() {
        let err = AdapterError::notification_not_found(5);
        assert_eq!(err.code, ErrorCode::NotificationNotFound);
        assert!(err.message.contains("5"));
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn stale_ref_suggestion_is_transport_neutral() {
        let err = AdapterError::stale_ref("@e7");
        assert_eq!(err.code, ErrorCode::StaleRef);
        assert!(err.message.contains("@e7"));
        let suggestion = err
            .suggestion
            .as_deref()
            .expect("stale_ref should carry a suggestion");
        assert!(
            suggestion.contains("snapshot"),
            "stale-ref suggestion should mention running a snapshot, got: {suggestion}"
        );
        assert!(
            suggestion.contains("FFI"),
            "stale-ref suggestion should include FFI transport guidance, got: {suggestion}"
        );
    }

    #[test]
    fn ambiguous_target_error_has_machine_readable_code() {
        let err = AdapterError::ambiguous_target("2 candidates matched");

        assert_eq!(err.code, ErrorCode::AmbiguousTarget);
        assert_eq!(err.code.as_str(), "AMBIGUOUS_TARGET");
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn policy_denied_suggestion_is_mode_aware() {
        let headless =
            AdapterError::policy_denied_for_policy("blocked", InteractionPolicy::headless());
        assert!(headless.suggestion.unwrap().contains("--headed"));

        let focus_fallback =
            AdapterError::policy_denied_for_policy("blocked", InteractionPolicy::focus_fallback());
        assert!(focus_fallback.suggestion.unwrap().contains("--headed"));
    }

    #[test]
    fn all_error_codes_as_str_and_serde_are_consistent() {
        let cases = [
            (ErrorCode::PermDenied, "PERM_DENIED"),
            (ErrorCode::ElementNotFound, "ELEMENT_NOT_FOUND"),
            (ErrorCode::AppNotFound, "APP_NOT_FOUND"),
            (ErrorCode::ActionFailed, "ACTION_FAILED"),
            (ErrorCode::ActionNotSupported, "ACTION_NOT_SUPPORTED"),
            (ErrorCode::StaleRef, "STALE_REF"),
            (ErrorCode::AmbiguousTarget, "AMBIGUOUS_TARGET"),
            (ErrorCode::WindowNotFound, "WINDOW_NOT_FOUND"),
            (ErrorCode::PlatformNotSupported, "PLATFORM_NOT_SUPPORTED"),
            (ErrorCode::Timeout, "TIMEOUT"),
            (ErrorCode::InvalidArgs, "INVALID_ARGS"),
            (ErrorCode::NotificationNotFound, "NOTIFICATION_NOT_FOUND"),
            (ErrorCode::SnapshotNotFound, "SNAPSHOT_NOT_FOUND"),
            (ErrorCode::PolicyDenied, "POLICY_DENIED"),
            (ErrorCode::Internal, "INTERNAL"),
        ];
        for (code, expected) in cases {
            assert_eq!(code.as_str(), expected, "as_str() mismatch for {expected}");
            let serialized = serde_json::to_string(&code).expect("serializable");
            assert_eq!(
                serialized,
                format!("\"{expected}\""),
                "serde output mismatch for {expected}"
            );
        }
    }

    #[test]
    fn non_adapter_app_errors_yield_internal_code_and_no_suggestion() {
        let io_err = AppError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "missing"));
        let json_err =
            AppError::Json(serde_json::from_str::<serde_json::Value>("not json").unwrap_err());
        let internal_err = AppError::Internal("unexpected state".into());

        for err in [&io_err, &json_err, &internal_err] {
            assert_eq!(
                err.code(),
                "INTERNAL",
                "expected INTERNAL code, got {}",
                err.code()
            );
            assert!(
                err.suggestion().is_none(),
                "non-adapter AppError must not carry a suggestion"
            );
        }
    }
}
