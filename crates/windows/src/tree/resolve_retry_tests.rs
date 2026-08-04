//! The deadline retry loop's own tests: settled never retries, incomplete
//! retries within budget, and expiry carries the last diagnosis.

use super::*;

fn retryable_incomplete(message: &str) -> AdapterError {
    AdapterError::new(agent_desktop_core::ErrorCode::AppUnresponsive, message)
        .with_details(serde_json::json!({ "retryable": true, "complete": false }))
}

fn err_code(result: Result<NativeHandle, AdapterError>) -> ErrorCode {
    match result {
        Err(error) => error.code,
        Ok(_) => panic!("expected an error, got a resolved handle"),
    }
}

fn err_code_owned(result: Result<NativeHandle, AdapterError>) -> AdapterError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected an error, got a resolved handle"),
    }
}

fn short_deadline() -> Deadline {
    Deadline::after(200).expect("a deadline")
}

fn generous_deadline() -> Deadline {
    Deadline::after(5_000).expect("a deadline")
}

/// An incomplete attempt retries within its deadline and succeeds when the
/// underlying cause recovers - the tree stabilises after a vanishing
/// node, the fake recovers.
#[test]
fn an_incomplete_attempt_retries_and_succeeds_within_the_deadline() {
    let mut attempts = 0;
    let deadline = generous_deadline();
    let result = retry_incomplete_until(deadline, || {
        attempts += 1;
        if attempts < 3 {
            Err(retryable_incomplete("transient"))
        } else {
            Ok(unreachable_handle())
        }
    });
    assert!(result.is_ok());
    assert_eq!(attempts, 3);
}

/// A settled non-match (a completed search that finds nothing) is never
/// retried - the call-count pin fails if the classification is loosened.
#[test]
fn a_settled_non_match_never_retries() {
    let mut attempts = 0;
    let deadline = generous_deadline();
    let result = retry_incomplete_until(deadline, || {
        attempts += 1;
        Err(agent_desktop_core::AdapterError::stale_ref("nothing")
            .with_details(serde_json::json!({ "complete": true, "retryable": true })))
    });
    assert_eq!(err_code(result), agent_desktop_core::ErrorCode::StaleRef);
    assert_eq!(attempts, 1);
}

/// An unresponsive error that was not stamped retryable is not retried: the
/// loop's `is_retryable_resolution_error` requires the explicit stamp, so a
/// raw transport error terminates rather than burning the budget guessing.
#[test]
fn an_unstamped_unresponsive_error_is_not_retried() {
    let mut attempts = 0;
    let deadline = generous_deadline();
    let result = retry_incomplete_until(deadline, || {
        attempts += 1;
        Err(AdapterError::new(
            agent_desktop_core::ErrorCode::AppUnresponsive,
            "raw",
        ))
    });
    assert_eq!(
        err_code(result),
        agent_desktop_core::ErrorCode::AppUnresponsive
    );
    assert_eq!(attempts, 1);
}

/// Deadline expiry mid-incompleteness returns the last incomplete
/// diagnosis stamped `deadline_elapsed`, not a bare `TIMEOUT` that
/// discards the diagnosis.
#[test]
fn deadline_expiry_mid_incompleteness_returns_the_last_diagnosis_stamped() {
    let deadline = short_deadline();
    let result = retry_incomplete_until(deadline, || Err(retryable_incomplete("stuck")));
    let error = err_code_owned(result);
    assert_eq!(error.code, agent_desktop_core::ErrorCode::AppUnresponsive);
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("deadline_elapsed"))
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

/// Expiry with no incomplete diagnosis returns the plain timeout - there
/// is nothing more informative to surface.
#[test]
fn deadline_expiry_with_no_incomplete_returns_the_timeout() {
    let deadline = short_deadline();
    let result = retry_incomplete_until(deadline, || {
        Err(AdapterError::new(
            agent_desktop_core::ErrorCode::Timeout,
            "gone",
        ))
    });
    assert_eq!(err_code(result), agent_desktop_core::ErrorCode::Timeout);
}

/// The deadline stamp and the typed details fields are the only places a
/// marker survives; message and `platform_detail` stay clean.
#[test]
fn the_deadline_stamp_leaks_no_marker_into_message_or_platform_detail() {
    let error = AdapterError::new(
        agent_desktop_core::ErrorCode::AppUnresponsive,
        "Strict resolution could not determine candidate identity",
    )
    .with_platform_detail("com-hresult-shape")
    .with_details(serde_json::json!({
        "kind": "resolution_identity_unknown",
        "secret_slot": "MARKER-9f2c",
        "complete": false,
        "retryable": true,
    }));

    let stamped = mark_deadline_elapsed(error);
    assert!(!stamped.message.contains("MARKER-9f2c"));
    assert!(
        !stamped
            .platform_detail
            .unwrap_or_default()
            .contains("MARKER-9f2c")
    );
    let details = stamped.details.expect("details preserved");
    assert_eq!(
        details
            .get("secret_slot")
            .and_then(serde_json::Value::as_str),
        Some("MARKER-9f2c")
    );
    assert_eq!(
        details
            .get("deadline_elapsed")
            .and_then(serde_json::Value::as_bool),
        Some(true)
    );
}

fn unreachable_handle() -> NativeHandle {
    NativeHandle::new(())
}
