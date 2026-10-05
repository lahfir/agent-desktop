use super::*;

#[test]
fn persistent_appkit_bridge_failure_preserves_operation_and_status() {
    let error = stabilize_global_with(
        Instant::now() + std::time::Duration::from_millis(20),
        || {
            Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "AppKit workspace snapshot failed",
            )
            .with_details(serde_json::json!({
                "kind": "appkit_bridge",
                "operation": "workspace_snapshot",
                "status": 2,
                "failure_field": "application_name",
                "failure_index": 7,
                "failure_pid": 123,
                "retryable": true,
            })))
        },
    )
    .unwrap_err();

    let details = error.details.unwrap();
    assert_eq!(details["last_kind"], "appkit_bridge");
    assert_eq!(details["last_operation"], "workspace_snapshot");
    assert_eq!(details["last_status"], 2);
    assert_eq!(details["last_failure_field"], "application_name");
    assert_eq!(details["last_failure_index"], 7);
    assert_eq!(details["last_failure_pid"], 123);
}

#[test]
fn a_failure_without_context_leaves_the_last_failure_fields_out() {
    let error = stabilize_global_with(
        Instant::now() + std::time::Duration::from_millis(20),
        || {
            Err(AdapterError::new(
                ErrorCode::AppUnresponsive,
                "AppKit workspace snapshot failed",
            )
            .with_details(serde_json::json!({
                "kind": "appkit_bridge",
                "operation": "workspace_snapshot",
                "status": 1,
                "retryable": true,
            })))
        },
    )
    .unwrap_err();

    let details = error.details.unwrap();
    for key in [
        "last_failure_field",
        "last_failure_index",
        "last_failure_pid",
    ] {
        assert!(details.get(key).is_none(), "{key} is omitted, not null");
    }
    assert_eq!(details["last_status"], 1);
}
