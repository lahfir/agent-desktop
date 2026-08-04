use super::{
    CO_E_NOTINITIALIZED, E_ACCESSDENIED, E_FAIL, E_INVALIDARG, E_NOINTERFACE, E_POINTER,
    RPC_E_DISCONNECTED, RPC_E_SERVERFAULT, RPC_S_CALL_FAILED, RPC_S_SERVER_UNAVAILABLE,
    ReadDisposition, UIA_E_ELEMENTNOTAVAILABLE, UIA_E_ELEMENTNOTENABLED, UIA_E_INVALIDOPERATION,
    UIA_E_NOCLICKABLEPOINT, UIA_E_NOTSUPPORTED, UIA_E_PROXYASSEMBLYNOTLOADED, UIA_E_TIMEOUT,
    classify_read_hresult, hresult_record,
};
use crate::tree::automation::{
    ERR_INVALID_ARG, ERR_NOTFOUND, ERR_TIMEOUT, UiaFailure, root_resolution_error,
    uia_failure_disposition, uia_failure_error,
};
use agent_desktop_core::ErrorCode;

/// The three-way read-path disposition is exercised arm by arm: the
/// not-supported family is a settled absence (never retried), transport is
/// retryable, a vanished element is the granularity case, and the stamp on
/// `uia_failure_error` carries the typed `complete`/`retryable` pair.
#[test]
fn not_supported_is_a_settled_absence_never_retried() {
    assert_eq!(
        classify_read_hresult(UIA_E_NOTSUPPORTED),
        ReadDisposition::SettledAbsence
    );
    let error = uia_failure_error(UiaFailure::Hresult(UIA_E_NOTSUPPORTED), "read a property");
    assert!(!error.is_explicitly_retryable());
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("complete"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

#[test]
fn invalid_argument_is_a_structurally_impossible_settled_absence() {
    assert_eq!(
        classify_read_hresult(E_INVALIDARG),
        ReadDisposition::SettledAbsence
    );
}

#[test]
fn transport_and_timeout_are_retryable() {
    for hresult in [UIA_E_TIMEOUT, RPC_E_DISCONNECTED] {
        assert_eq!(classify_read_hresult(hresult), ReadDisposition::Retryable);
    }
    let error = uia_failure_error(UiaFailure::Hresult(RPC_E_DISCONNECTED), "walk a sibling");
    assert!(error.is_explicitly_retryable());
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}

#[test]
fn a_vanished_element_is_unavailable_for_the_granularity_split() {
    assert_eq!(
        classify_read_hresult(UIA_E_ELEMENTNOTAVAILABLE),
        ReadDisposition::Unavailable
    );
}

#[test]
fn the_sentinel_branches_classify_like_their_hresult_equivalents() {
    assert_eq!(
        uia_failure_disposition(UiaFailure::Sentinel(ERR_TIMEOUT)),
        ReadDisposition::Retryable
    );
    assert_eq!(
        uia_failure_disposition(UiaFailure::Sentinel(ERR_NOTFOUND)),
        ReadDisposition::Unavailable
    );
    assert_eq!(
        uia_failure_disposition(UiaFailure::Sentinel(ERR_INVALID_ARG)),
        ReadDisposition::SettledAbsence
    );
}

/// The A14-5 split is carried: at root resolution the vanished-window shape
/// stays `WINDOW_NOT_FOUND`, while the same HRESULT means a stale element on
/// the read path (the live-read path resolves the granularity at the target).
#[test]
fn the_vanished_element_split_keeps_root_resolution_as_window_not_found() {
    let failure = UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE);
    assert_eq!(
        root_resolution_error(failure).code,
        ErrorCode::WindowNotFound
    );
}

/// `classify_read_hresult` is a one-line projection of `hresult_record`
/// rather than an independent match, walked here over every named HRESULT so
/// a future edit that reintroduces a second, separately-maintained match
/// cannot drift from the record without failing this test.
#[test]
fn classify_read_hresult_never_drifts_from_the_single_record() {
    for hresult in [
        E_NOINTERFACE,
        E_POINTER,
        E_FAIL,
        E_ACCESSDENIED,
        E_INVALIDARG,
        CO_E_NOTINITIALIZED,
        RPC_E_SERVERFAULT,
        RPC_E_DISCONNECTED,
        RPC_S_SERVER_UNAVAILABLE,
        RPC_S_CALL_FAILED,
        UIA_E_ELEMENTNOTENABLED,
        UIA_E_ELEMENTNOTAVAILABLE,
        UIA_E_NOCLICKABLEPOINT,
        UIA_E_PROXYASSEMBLYNOTLOADED,
        UIA_E_NOTSUPPORTED,
        UIA_E_TIMEOUT,
        UIA_E_INVALIDOPERATION,
    ] {
        assert_eq!(
            classify_read_hresult(hresult),
            hresult_record(hresult).disposition,
            "0x{hresult:08X} disagreed with its own record"
        );
    }
}

/// The pairing this crate's read path leans on for `UIA_E_ELEMENTNOTAVAILABLE`
/// specifically: a vanished target must carry both `ErrorCode::StaleRef` and
/// the `Unavailable` disposition together, from the one record, not from two
/// tables that happen to agree.
#[test]
fn a_vanished_elements_record_pairs_stale_ref_with_the_unavailable_disposition() {
    let record = hresult_record(UIA_E_ELEMENTNOTAVAILABLE);
    assert_eq!(record.code, ErrorCode::StaleRef);
    assert_eq!(record.disposition, ReadDisposition::Unavailable);
}
