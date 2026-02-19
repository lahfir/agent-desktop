use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    PermDenied,
    ElementNotFound,
    AppNotFound,
    ActionFailed,
    ActionNotSupported,
    StaleRef,
    WindowNotFound,
    PlatformNotSupported,
    Timeout,
    InvalidArgs,
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
            ErrorCode::WindowNotFound => "WINDOW_NOT_FOUND",
            ErrorCode::PlatformNotSupported => "PLATFORM_NOT_SUPPORTED",
            ErrorCode::Timeout => "TIMEOUT",
            ErrorCode::InvalidArgs => "INVALID_ARGS",
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

    pub fn stale_ref(ref_id: &str) -> Self {
        Self::new(ErrorCode::StaleRef, format!("{ref_id} not found in current RefMap"))
            .with_suggestion("Run 'snapshot' to refresh, then retry with updated ref")
    }

    pub fn not_supported(method: &str) -> Self {
        Self::new(
            ErrorCode::PlatformNotSupported,
            format!("{method} is not supported on this platform"),
        )
        .with_suggestion("This platform adapter ships in Phase 2")
    }

    pub fn element_not_found(ref_id: &str) -> Self {
        Self::new(ErrorCode::ElementNotFound, format!("Element {ref_id} could not be resolved"))
            .with_suggestion("Run 'snapshot' to get fresh refs")
    }

    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Timeout, msg)
            .with_suggestion("The target application may be busy or unresponsive")
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, msg)
    }

    pub fn permission_denied() -> Self {
        Self::new(ErrorCode::PermDenied, "Accessibility permission not granted")
            .with_suggestion(
                "Open System Settings > Privacy & Security > Accessibility and add your terminal",
            )
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
}
