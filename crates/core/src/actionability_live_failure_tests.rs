use super::*;

#[test]
fn empty_live_actions_replace_stale_snapshot_capabilities() {
    let stale = entry();
    let adapter = LiveAdapter {
        state: None,
        bounds: stale.geometry.bounds,
        actions: Some(vec![]),
    };

    let err = check_live(
        &stale,
        &NativeHandle::null(),
        &adapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(err.code, ErrorCode::PolicyDenied);
    assert!(err.message.contains("supported_action"));
}

#[test]
fn unsupported_live_reads_fail_closed() {
    let err = check_live(
        &entry(),
        &NativeHandle::null(),
        &UnsupportedLiveAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(err.code, ErrorCode::ActionNotSupported);
    assert!(err.message.contains("Live element evidence"));
}

#[test]
fn empty_live_element_fails_as_stale_before_dispatch() {
    let err = check_live(
        &entry(),
        &NativeHandle::null(),
        &DeadLiveElementAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(err.code, ErrorCode::StaleRef);
    assert!(err.message.contains("changed role"));
}

#[test]
fn live_read_errors_are_not_silently_downgraded_to_snapshot_data() {
    let err = check_live(
        &entry(),
        &NativeHandle::null(),
        &LiveReadErrorAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(err.code, ErrorCode::PermDenied);
}
