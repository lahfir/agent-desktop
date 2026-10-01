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
                "retryable": true,
            })))
        },
    )
    .unwrap_err();

    let details = error.details.unwrap();
    assert_eq!(details["last_kind"], "appkit_bridge");
    assert_eq!(details["last_operation"], "workspace_snapshot");
    assert_eq!(details["last_status"], 2);
}
