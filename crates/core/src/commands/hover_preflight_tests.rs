use super::*;

struct OccludedTargetAdapter {
    moved_to: Mutex<Option<MouseEvent>>,
}

impl ObservationOps for OccludedTargetAdapter {
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
            x: 100.0,
            y: 200.0,
            width: 20.0,
            height: 10.0,
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

impl ActionOps for OccludedTargetAdapter {}

impl InputOps for OccludedTargetAdapter {
    fn mouse_event(
        &self,
        event: MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<(), AdapterError> {
        *self.moved_to.lock().unwrap() = Some(event);
        Ok(())
    }
}

impl SystemOps for OccludedTargetAdapter {
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

/// F27 regression: `hover --ref` previously resolved the ref's bounds to a
/// point and dispatched the mouse move without ever consulting `hit_test`,
/// so an occluded target (e.g. a modal sheet over it) was hovered blind.
/// This proves the preflight now fails before any mouse event is sent.
#[test]
fn hover_on_occluded_ref_fails_preflight_before_dispatch() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = OccludedTargetAdapter {
        moved_to: Mutex::new(None),
    };

    let err = execute(
        ref_args(snapshot_id),
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
    assert!(adapter.moved_to.lock().unwrap().is_none());
}

#[test]
fn timeout_none_uses_the_default_retry_budget() {
    let _guard = HomeGuard::new();
    let snapshot_id = ref_snapshot(42);
    let adapter = StaleThenOkAdapter::new(1);

    let value = execute(
        HoverArgs {
            ref_id: Some("@e1".into()),
            snapshot_id: Some(snapshot_id),
            xy: None,
            duration_ms: None,
            timeout_ms: None,
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap();

    assert_eq!(value["hovered"], true);
    assert!(adapter.retry.calls() >= 2);
}

#[test]
fn expired_xy_budget_never_dispatches_mouse_move() {
    let adapter = HoverCaptureAdapter::new();

    let err = execute(
        HoverArgs {
            ref_id: None,
            snapshot_id: None,
            xy: Some((5.0, 6.0)),
            duration_ms: None,
            timeout_ms: Some(0),
        },
        &adapter,
        &CommandContext::default().with_headed(true),
    )
    .unwrap_err();

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
    assert!(adapter.moved_to.lock().unwrap().is_none());
}
