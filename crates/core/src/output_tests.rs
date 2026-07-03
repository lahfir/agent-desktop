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
fn app_unresponsive_payload_carries_a_recovery_token() {
    let err = AppError::Adapter(AdapterError::app_unresponsive("Finder"));
    let payload = ErrorPayload::from_app_error(&err);
    assert_eq!(payload.code, "APP_UNRESPONSIVE");
    assert_eq!(
        payload.retry_command.as_deref(),
        Some("wait;execute_by_ref"),
        "APP_UNRESPONSIVE must carry a sensible recovery token, not silently omit one"
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

#[test]
fn ok_response_json_shape_has_version_ok_command_data_and_no_error_field() {
    let resp = Response::ok("snapshot", json!({"app": "Finder"}));
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_value(serde_json::to_value(&resp).expect("serializable")).expect("map");

    assert_eq!(map["version"].as_str(), Some("2.1"), "version must be 2.1");
    assert_eq!(map["ok"].as_bool(), Some(true), "ok must be true");
    assert_eq!(
        map["command"].as_str(),
        Some("snapshot"),
        "command must match"
    );
    assert!(map.contains_key("data"), "ok response must have data field");
    assert!(
        !map.contains_key("error"),
        "ok response must not serialize an error field (skip_serializing_if = is_none)"
    );
}

#[test]
fn err_response_json_shape_has_version_ok_command_error_and_no_data_field() {
    let payload =
        ErrorPayload::new("STALE_REF", "ref @e1 is stale").with_suggestion("re-run snapshot");
    let resp = Response::err("click", payload);
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_value(serde_json::to_value(&resp).expect("serializable")).expect("map");

    assert_eq!(map["version"].as_str(), Some("2.1"), "version must be 2.1");
    assert_eq!(map["ok"].as_bool(), Some(false), "ok must be false");
    assert_eq!(map["command"].as_str(), Some("click"), "command must match");
    assert!(
        !map.contains_key("data"),
        "err response must not serialize a data field (skip_serializing_if = is_none)"
    );
    assert!(
        map.contains_key("error"),
        "err response must have error field"
    );
    assert_eq!(
        map["error"]["code"].as_str(),
        Some("STALE_REF"),
        "error code must round-trip"
    );
    assert_eq!(
        map["error"]["message"].as_str(),
        Some("ref @e1 is stale"),
        "error message must round-trip"
    );
    assert_eq!(
        map["error"]["suggestion"].as_str(),
        Some("re-run snapshot"),
        "suggestion must be present when set"
    );
}

#[test]
fn err_response_omits_optional_error_subfields_when_absent() {
    let payload = ErrorPayload::new("INTERNAL", "something broke");
    let resp = Response::err("snapshot", payload);
    let map: serde_json::Map<String, serde_json::Value> =
        serde_json::from_value(serde_json::to_value(&resp).expect("serializable")).expect("map");

    let error = map["error"].as_object().expect("error must be an object");
    assert!(
        !error.contains_key("suggestion"),
        "absent suggestion must be omitted from JSON"
    );
    assert!(
        !error.contains_key("retry_command"),
        "absent retry_command must be omitted from JSON"
    );
    assert!(
        !error.contains_key("platform_detail"),
        "absent platform_detail must be omitted from JSON"
    );
    assert!(
        !error.contains_key("details"),
        "absent details must be omitted from JSON"
    );
}
