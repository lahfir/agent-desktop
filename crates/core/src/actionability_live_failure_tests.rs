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
    assert_eq!(err.disposition, crate::DeliverySemantics::not_delivered());
}

#[test]
fn transient_live_read_failure_reports_no_delivery() {
    struct FailedRead(crate::DeliverySemantics);
    impl ObservationOps for FailedRead {
        fn get_live_element(
            &self,
            _: &NativeHandle,
            _: crate::Deadline,
        ) -> Result<LiveElement, AdapterError> {
            Err(
                AdapterError::new(ErrorCode::AppUnresponsive, "incomplete live evidence")
                    .with_disposition(self.0),
            )
        }
    }
    impl ActionOps for FailedRead {}
    impl InputOps for FailedRead {}
    impl SystemOps for FailedRead {}

    for (reported, expected) in [
        (
            crate::DeliverySemantics::unknown(),
            crate::DeliverySemantics::not_delivered(),
        ),
        (
            crate::DeliverySemantics::uncertain(),
            crate::DeliverySemantics::uncertain(),
        ),
    ] {
        let error = check_live(
            &entry(),
            &NativeHandle::null(),
            &FailedRead(reported),
            &ActionRequest::headless(Action::Click),
        )
        .unwrap_err();
        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(error.disposition, expected);
    }
}
