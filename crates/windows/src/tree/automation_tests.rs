use super::*;

const E_ACCESSDENIED_HRESULT: i32 = 0x8007_0005_u32 as i32;
const UIA_E_ELEMENTNOTAVAILABLE_HRESULT: i32 = 0x8004_0201_u32 as i32;
const RPC_S_SERVER_UNAVAILABLE_HRESULT: i32 = 0x8007_06BA_u32 as i32;
const UNRECOGNISED_SENTINEL: i32 = 4242;
const LEAK_MARKER: &str = "zzmarkerzz-secret-window-title";

#[test]
fn a_negative_hresult_formats_with_its_symbolic_name() {
    let error = uia_failure_error(
        UiaFailure::Hresult(E_ACCESSDENIED_HRESULT),
        "read a property",
    );

    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(
        error.platform_detail.as_deref(),
        Some("COM HRESULT 0x80070005 (E_ACCESSDENIED: Access is denied)")
    );
}

#[test]
fn a_crate_sentinel_never_prints_a_fabricated_hresult() {
    let error = uia_failure_error(UiaFailure::Sentinel(ERR_TIMEOUT), "read a property");

    assert_eq!(error.code, ErrorCode::Timeout);
    let detail = error.platform_detail.expect("a sentinel carries detail");
    assert!(!detail.contains("0x"), "sentinel detail was {detail}");
    assert!(!error.message.contains("0x"));
}

#[test]
fn an_unrecognised_sentinel_maps_to_internal_rather_than_a_guess() {
    let error = uia_failure_error(
        UiaFailure::Sentinel(UNRECOGNISED_SENTINEL),
        "walk a subtree",
    );

    assert_eq!(error.code, ErrorCode::Internal);
}

#[test]
fn every_named_sentinel_maps_without_a_catch_all_guess() {
    assert_eq!(
        sentinel_disposition(ERR_NOTFOUND),
        ErrorCode::ElementNotFound
    );
    assert_eq!(
        sentinel_disposition(ERR_NULL_PTR),
        ErrorCode::ElementNotFound
    );
    assert_eq!(sentinel_disposition(ERR_TIMEOUT), ErrorCode::Timeout);
    assert_eq!(
        sentinel_disposition(ERR_INACTIVE),
        ErrorCode::AppUnresponsive
    );
    assert_eq!(
        sentinel_disposition(ERR_INVALID_OBJECT),
        ErrorCode::StaleRef
    );
    assert_eq!(
        sentinel_disposition(ERR_INVALID_ARG),
        ErrorCode::InvalidArgs
    );
    for internal in [ERR_NONE, ERR_TYPE, ERR_FORMAT, ERR_ALREADY_RUNNING] {
        assert_eq!(sentinel_disposition(internal), ErrorCode::Internal);
    }
}

#[test]
fn a_disconnected_provider_reports_an_unresponsive_app() {
    let error = uia_failure_error(
        UiaFailure::Hresult(RPC_S_SERVER_UNAVAILABLE_HRESULT),
        "enumerate children",
    );

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}

#[test]
fn only_the_empty_sentinel_counts_as_exhaustion() {
    assert!(UiaFailure::Sentinel(ERR_NONE).is_exhaustion());
    assert!(!UiaFailure::Sentinel(ERR_NOTFOUND).is_exhaustion());
    assert!(!UiaFailure::Hresult(0).is_exhaustion());
    assert!(!UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE_HRESULT).is_exhaustion());
}

#[test]
fn an_unavailable_root_is_a_missing_window_not_a_stale_element() {
    let by_hresult = root_resolution_error(UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE_HRESULT));
    let by_sentinel = root_resolution_error(UiaFailure::Sentinel(ERR_NOTFOUND));

    assert_eq!(by_hresult.code, ErrorCode::WindowNotFound);
    assert_eq!(by_sentinel.code, ErrorCode::WindowNotFound);
    assert!(
        by_hresult
            .platform_detail
            .is_some_and(|detail| detail.contains("UIA_E_ELEMENTNOTAVAILABLE"))
    );
}

#[test]
fn a_root_failure_that_is_not_a_missing_window_keeps_its_own_code() {
    let error = root_resolution_error(UiaFailure::Hresult(E_ACCESSDENIED_HRESULT));

    assert_eq!(error.code, ErrorCode::PermDenied);
}

/// KTD14: `ref_action.rs` clones adapter error text into session JSONL and
/// into the trace HTML export, so a marker that reaches the caller through
/// the context phrase must not reach the persisted error.
#[test]
fn a_classifier_error_carries_shape_and_never_target_content() {
    let error = uia_failure_error(
        UiaFailure::Hresult(E_ACCESSDENIED_HRESULT),
        "read a property",
    );
    let rendered = format!(
        "{}|{}|{}",
        error.message,
        error.platform_detail.clone().unwrap_or_default(),
        serde_json::to_string(&error.details).unwrap_or_default()
    );

    assert!(!rendered.contains(LEAK_MARKER), "leaked: {rendered}");
    assert!(!rendered.to_ascii_lowercase().contains("zzmarkerzz"));
}

#[test]
fn an_empty_window_handle_is_rejected_before_the_com_layer() {
    let deadline = Deadline::standard().expect("a standard deadline");

    let Err(error) = validate_window_handle(0, deadline) else {
        panic!("an empty window handle must be rejected");
    };

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(validate_window_handle(1, deadline).is_ok());
}

#[cfg(not(target_os = "windows"))]
#[test]
fn the_canned_resolver_reports_a_missing_window_for_an_empty_handle() {
    let deadline = Deadline::standard().expect("a standard deadline");

    let Err(error) = root_from_hwnd(0, deadline) else {
        panic!("the canned arm must reject an empty handle");
    };

    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert!(root_from_hwnd(1, deadline).is_ok());
}

#[cfg(target_os = "windows")]
mod windows_only {
    use super::*;
    use uiautomation::Error as UiaError;

    fn bootstrap() {
        crate::tree::fixture::ensure_test_apartment();
    }

    #[test]
    fn the_client_is_reusable_within_a_thread() {
        bootstrap();

        assert!(automation_client().is_ok());
        assert!(automation_client().is_ok());
    }

    #[test]
    fn a_hresult_error_splits_onto_the_result_branch() {
        let error = UiaError::from(windows::core::HRESULT(E_ACCESSDENIED_HRESULT));

        assert_eq!(
            failure_of(&error),
            UiaFailure::Hresult(E_ACCESSDENIED_HRESULT)
        );
    }

    #[test]
    fn a_sentinel_error_splits_onto_the_sentinel_branch() {
        let error = UiaError::new(ERR_TIMEOUT, "");

        assert_eq!(failure_of(&error), UiaFailure::Sentinel(ERR_TIMEOUT));
    }

    /// The runtime end-of-list pair the walker depends on, asserted against
    /// the real crate rather than against a reading of it.
    #[test]
    fn a_null_interface_out_param_reports_the_exhaustion_sentinel() {
        let error = UiaError::from(windows::core::Error::empty());

        assert!(failure_of(&error).is_exhaustion());
    }

    #[test]
    fn a_destroyed_window_handle_reports_a_missing_window() {
        bootstrap();
        let deadline = Deadline::standard().expect("a standard deadline");

        let Err(error) = root_from_hwnd(isize::MAX, deadline) else {
            panic!("a handle that addresses no window must not resolve");
        };

        assert!(matches!(
            error.code,
            ErrorCode::WindowNotFound | ErrorCode::InvalidArgs | ErrorCode::ElementNotFound
        ));
    }

    #[test]
    fn a_live_uia_error_never_carries_the_crate_message_verbatim() {
        let error = uia_error(&UiaError::new(ERR_TIMEOUT, LEAK_MARKER), "read a property");

        assert!(!error.message.contains(LEAK_MARKER));
        assert!(
            !error
                .platform_detail
                .unwrap_or_default()
                .contains(LEAK_MARKER)
        );
    }
}
