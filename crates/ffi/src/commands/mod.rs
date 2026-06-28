pub(crate) mod envelope_out;
pub(crate) mod generated;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adapter_variant_passes_through_with_original_code_and_message() {
        let ae = AdapterError::stale_ref("@e42");
        let original_code = ae.code.clone();
        let original_msg = ae.message.clone();
        let result = app_error_to_adapter(AppError::Adapter(ae));
        assert_eq!(result.code, original_code);
        assert_eq!(result.message, original_msg);
    }

    #[test]
    fn io_error_collapses_to_internal_and_embeds_os_message() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file gone");
        let result = app_error_to_adapter(AppError::Io(io_err));
        assert_eq!(result.code, ErrorCode::Internal);
        assert!(
            result.message.contains("file gone"),
            "IO message must be forwarded; got: {:?}",
            result.message
        );
    }

    #[test]
    fn json_error_collapses_to_internal_with_non_empty_message() {
        let json_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
        let result = app_error_to_adapter(AppError::Json(json_err));
        assert_eq!(result.code, ErrorCode::Internal);
        assert!(
            !result.message.is_empty(),
            "JSON parse error message must be forwarded"
        );
    }

    #[test]
    fn internal_string_collapses_to_internal_preserving_exact_message() {
        let result = app_error_to_adapter(AppError::Internal("unexpected state".into()));
        assert_eq!(result.code, ErrorCode::Internal);
        assert_eq!(result.message, "unexpected state");
    }
}
