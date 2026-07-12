use super::*;

struct StaleThenOkAdapter {
    retry: StaleRetryCounter,
}

impl StaleThenOkAdapter {
    fn new(fail_until: u32) -> Self {
        Self {
            retry: StaleRetryCounter::new(fail_until),
        }
    }
}

impl ObservationOps for StaleThenOkAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.retry.attempt()
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 60.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::ReachesTarget)
    }
}

impl ActionOps for StaleThenOkAdapter {}

impl InputOps for StaleThenOkAdapter {
    fn drag(
        &self,
        _params: DragParams,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }
}

impl SystemOps for StaleThenOkAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        window: &crate::WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<crate::WindowInfo, AdapterError> {
        Ok(window.clone())
    }

    fn focus_window(
        &self,
        _window: &crate::WindowInfo,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }
}

#[test]
fn transient_stale_ref_retries_then_succeeds_when_timeout_wired() {
    let _guard = HomeGuard::new();
    let snapshot_id = cross_app_snapshot();
    let adapter = StaleThenOkAdapter::new(2);

    let value = execute(
        DragArgs {
            from_ref: Some("@e1".into()),
            from_xy: None,
            to_ref: Some("@e2".into()),
            to_xy: None,
            snapshot_id: Some(snapshot_id),
            duration_ms: None,
            drop_delay_ms: None,
            timeout_ms: Some(5_000),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(value["dragged"], true);
    assert!(adapter.retry.calls() >= 3);
}

struct OccludedFromAdapter {
    captured: Mutex<Option<DragParams>>,
}

impl ObservationOps for OccludedFromAdapter {
    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Ok(Some(Rect {
            x: 10.0,
            y: 20.0,
            width: 40.0,
            height: 60.0,
        }))
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<HitTestResult, AdapterError> {
        Ok(HitTestResult::InterceptedBy {
            role: Some("AXSheet".into()),
            name: Some("Save changes?".into()),
            bounds: None,
        })
    }
}

impl ActionOps for OccludedFromAdapter {}

impl InputOps for OccludedFromAdapter {
    fn drag(
        &self,
        params: DragParams,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        *self.captured.lock().unwrap() = Some(params);
        Ok(())
    }
}

impl SystemOps for OccludedFromAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        window: &crate::WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<crate::WindowInfo, AdapterError> {
        Ok(window.clone())
    }

    fn focus_window(
        &self,
        _window: &crate::WindowInfo,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        Ok(())
    }
}

#[test]
fn drag_from_occluded_ref_fails_preflight_before_dispatch() {
    let _guard = HomeGuard::new();
    let snapshot_id = cross_app_snapshot();
    let adapter = OccludedFromAdapter {
        captured: Mutex::new(None),
    };

    let err = execute(
        cross_app_args(snapshot_id),
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    let AppError::Adapter(adapter_error) = &err else {
        panic!("expected adapter timeout")
    };
    assert!(
        adapter_error
            .details
            .as_ref()
            .is_some_and(|details| details.to_string().contains("AXSheet"))
    );
    assert!(adapter.captured.lock().unwrap().is_none());
}

#[test]
fn timeout_none_uses_the_default_retry_budget() {
    let _guard = HomeGuard::new();
    let snapshot_id = cross_app_snapshot();
    let adapter = StaleThenOkAdapter::new(1);

    let value = execute(
        DragArgs {
            from_ref: Some("@e1".into()),
            from_xy: None,
            to_ref: Some("@e2".into()),
            to_xy: None,
            snapshot_id: Some(snapshot_id),
            duration_ms: None,
            drop_delay_ms: None,
            timeout_ms: None,
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(value["dragged"], true);
    assert!(adapter.retry.calls() >= 2);
}

#[test]
fn expired_xy_budget_never_dispatches_drag() {
    let adapter = DragCaptureAdapter::new();
    let mut args = xy_args(None);
    args.timeout_ms = Some(0);

    let err = execute(args, &adapter, &CommandContext::default().with_headed(true)).unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    let AppError::Adapter(adapter_error) = &err else {
        panic!("expected adapter timeout")
    };
    assert_eq!(
        adapter_error
            .details
            .as_ref()
            .and_then(|details| details.get("kind")),
        Some(&serde_json::json!("actionability_timeout"))
    );
    assert!(adapter.captured.lock().unwrap().is_none());
}
