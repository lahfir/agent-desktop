pub mod snapshot;
pub(crate) mod status;
pub(crate) mod version;

use agent_desktop_core::error::{AdapterError, AppError, ErrorCode};

/// Converts a core `AppError` into an `AdapterError` for use with
/// `set_last_error`. `AppError::Adapter` is already an `AdapterError`;
/// the other variants wrap their payload as `ErrorCode::Internal`.
pub(crate) fn app_error_to_adapter(err: AppError) -> AdapterError {
    match err {
        AppError::Adapter(e) => e,
        AppError::Io(e) => AdapterError::new(ErrorCode::Internal, e.to_string()),
        AppError::Json(e) => AdapterError::new(ErrorCode::Internal, e.to_string()),
        AppError::Internal(msg) => AdapterError::new(ErrorCode::Internal, msg),
    }
}
