use super::{classify_mutation, classify_success};
use crate::system::hresult::{
    E_ACCESSDENIED, E_INVALIDARG, RPC_E_DISCONNECTED, RPC_E_SERVERFAULT, RPC_S_CALL_FAILED,
    RPC_S_SERVER_UNAVAILABLE, UIA_E_ELEMENTNOTAVAILABLE, UIA_E_ELEMENTNOTENABLED,
    UIA_E_INVALIDOPERATION, UIA_E_NOTSUPPORTED, UIA_E_TIMEOUT,
};
use crate::tree::automation::{ERR_NONE, ERR_NOTFOUND, UiaFailure};
use agent_desktop_core::{DeliveryDisposition, ErrorCode, RetryDisposition};

fn hresult(code: i32) -> UiaFailure {
    UiaFailure::Hresult(code)
}

fn assert_err(
    failure: UiaFailure,
    code: ErrorCode,
    delivery: DeliveryDisposition,
    retry: RetryDisposition,
) {
    let error = classify_mutation("SetValue", "ValuePattern.SetValue", &failure)
        .expect_err("classified failure must be Err");
    assert_eq!(error.code, code);
    assert_eq!(error.disposition.delivery(), delivery);
    assert_eq!(error.disposition.retry(), retry);
}

#[test]
fn success_helper_is_delivered() {
    let result = classify_success().expect("success is Ok");
    assert!(result);
}

#[test]
fn not_supported_is_absence_never_err() {
    let result = classify_mutation(
        "Invoke",
        "InvokePattern.Invoke",
        &hresult(UIA_E_NOTSUPPORTED),
    )
    .expect("absence must not be Err");
    assert!(!result);
}

#[test]
fn empty_pattern_sentinel_is_absence_never_err() {
    let result = classify_mutation(
        "get_pattern",
        "UIScrollItemPattern",
        &UiaFailure::Sentinel(ERR_NONE),
    )
    .expect("empty-pattern sentinel must not be Err");
    assert!(!result);
}

#[test]
fn access_denied_is_perm_denied_not_delivered() {
    assert_err(
        hresult(E_ACCESSDENIED),
        ErrorCode::PermDenied,
        DeliveryDisposition::NotDelivered,
        RetryDisposition::Safe,
    );
}

#[test]
fn element_not_available_is_stale_not_delivered_with_refresh_suggestion() {
    let error = classify_mutation(
        "Invoke",
        "InvokePattern.Invoke",
        &hresult(UIA_E_ELEMENTNOTAVAILABLE),
    )
    .expect_err("stale element must fail closed");
    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
    assert!(
        error
            .suggestion
            .as_deref()
            .expect("stale carries a refresh suggestion")
            .contains("Refresh the snapshot")
    );
}

#[test]
fn invalid_arg_is_invalid_args_not_delivered() {
    assert_err(
        hresult(E_INVALIDARG),
        ErrorCode::InvalidArgs,
        DeliveryDisposition::NotDelivered,
        RetryDisposition::Safe,
    );
}

#[test]
fn element_not_enabled_is_action_failed_not_delivered() {
    assert_err(
        hresult(UIA_E_ELEMENTNOTENABLED),
        ErrorCode::ActionFailed,
        DeliveryDisposition::NotDelivered,
        RetryDisposition::Safe,
    );
}

#[test]
fn transport_server_fault_is_app_unresponsive_uncertain() {
    assert_err(
        hresult(RPC_E_SERVERFAULT),
        ErrorCode::AppUnresponsive,
        DeliveryDisposition::DeliveryUncertain,
        RetryDisposition::Unsafe,
    );
}

#[test]
fn transport_disconnected_is_app_unresponsive_uncertain() {
    assert_err(
        hresult(RPC_E_DISCONNECTED),
        ErrorCode::AppUnresponsive,
        DeliveryDisposition::DeliveryUncertain,
        RetryDisposition::Unsafe,
    );
}

#[test]
fn transport_server_unavailable_is_app_unresponsive_uncertain() {
    assert_err(
        hresult(RPC_S_SERVER_UNAVAILABLE),
        ErrorCode::AppUnresponsive,
        DeliveryDisposition::DeliveryUncertain,
        RetryDisposition::Unsafe,
    );
}

#[test]
fn transport_call_failed_is_app_unresponsive_uncertain() {
    assert_err(
        hresult(RPC_S_CALL_FAILED),
        ErrorCode::AppUnresponsive,
        DeliveryDisposition::DeliveryUncertain,
        RetryDisposition::Unsafe,
    );
}

#[test]
fn timeout_is_timeout_uncertain() {
    let error = classify_mutation(
        "ScrollIntoView",
        "ScrollItemPattern.ScrollIntoView",
        &hresult(UIA_E_TIMEOUT),
    )
    .expect_err("timeout must be Err");
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
    );
    assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
    assert!(
        error
            .suggestion
            .as_deref()
            .expect("uncertain carries inspect suggestion")
            .contains("Inspect the target state")
    );
}

#[test]
fn unclassified_hresult_is_action_failed_uncertain() {
    assert_err(
        hresult(UIA_E_INVALIDOPERATION),
        ErrorCode::ActionFailed,
        DeliveryDisposition::DeliveryUncertain,
        RetryDisposition::Unsafe,
    );
}

#[test]
fn unclassified_sentinel_is_action_failed_uncertain() {
    assert_err(
        UiaFailure::Sentinel(ERR_NOTFOUND),
        ErrorCode::ActionFailed,
        DeliveryDisposition::DeliveryUncertain,
        RetryDisposition::Unsafe,
    );
}

/// Bans every name that reaches the read-path classification table across every
/// mutation file. Spelled in halves so this file's own text cannot trip the
/// scan.
#[test]
fn write_path_sources_never_reach_the_read_classification_table() {
    let banned = [
        concat!("classify_read_", "hresult"),
        concat!("hresult_", "record"),
        concat!("uia_failure_", "disposition"),
    ];
    let sources = mutation_sources();
    for (name, source) in sources {
        for line in source.lines() {
            let is_prose =
                line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
            for banned_name in banned {
                assert!(
                    is_prose || !line.contains(banned_name),
                    "{name} must not consult {banned_name}: {line}"
                );
            }
        }
    }
    assert!(
        include_str!("mutation.rs").contains("com_hresult_detail"),
        "failed writes format HRESULT detail through com_hresult_detail"
    );
}

fn mutation_sources() -> [(&'static str, &'static str); 13] {
    [
        ("actions/mutation.rs", include_str!("mutation.rs")),
        (
            "actions/scroll_into_view.rs",
            include_str!("scroll_into_view.rs"),
        ),
        (
            "actions/scroll_ladder.rs",
            include_str!("scroll_ladder.rs"),
        ),
        ("actions/dispatch.rs", include_str!("dispatch.rs")),
        ("actions/focus.rs", include_str!("focus.rs")),
        ("actions/chain.rs", include_str!("chain.rs")),
        ("actions/value_write.rs", include_str!("value_write.rs")),
        ("actions/post_state.rs", include_str!("post_state.rs")),
        ("actions/toggle_state.rs", include_str!("toggle_state.rs")),
        ("actions/disclosure.rs", include_str!("disclosure.rs")),
        ("actions/select.rs", include_str!("select.rs")),
        ("actions/select_search.rs", include_str!("select_search.rs")),
        ("actions/scroll.rs", include_str!("scroll.rs")),
    ]
}

/// The stale arm must be built with `AdapterError::new(ErrorCode::StaleRef, …)`
/// so it never inherits `stale_ref`'s RefMap-shaped message. A19-2's killed
/// provider lands this arm.
#[test]
fn actions_never_construct_stale_via_adapter_error_stale_ref() {
    let banned = concat!("AdapterError::", "stale_ref");
    let sources = mutation_sources();
    for (name, source) in sources {
        for line in source.lines() {
            let is_prose =
                line.trim_start().starts_with("///") || line.trim_start().starts_with("//!");
            assert!(
                is_prose || !line.contains(banned),
                "{name} must not call {banned}: {line}"
            );
        }
    }
    assert!(
        include_str!("mutation.rs").contains("ErrorCode::StaleRef"),
        "the stale arm must name ErrorCode::StaleRef directly"
    );
}
