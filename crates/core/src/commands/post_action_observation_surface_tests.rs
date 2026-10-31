use super::*;

struct SurfaceAbsenceAdapter {
    inner: ProbeAdapter,
    error: AdapterError,
    fail_after_observations: usize,
}

impl ObservationOps for SurfaceAbsenceAdapter {
    fn is_application_surface(&self, surface: crate::SnapshotSurface) -> bool {
        surface == crate::SnapshotSurface::Menu
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        assert_eq!(request.surface, crate::SnapshotSurface::Menu);
        if self.inner.observed.lock().unwrap().len() >= self.fail_after_observations {
            return Err(self.error.clone());
        }
        self.inner.observe_tree(root, request)
    }
}

impl ActionOps for SurfaceAbsenceAdapter {}
impl InputOps for SurfaceAbsenceAdapter {}
impl SystemOps for SurfaceAbsenceAdapter {
    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        self.inner.resolve_window_strict(expected, deadline)
    }

    fn process_state(
        &self,
        process: ProcessIdentity,
        deadline: Deadline,
    ) -> Result<ProcessState, AdapterError> {
        self.inner.process_state(process, deadline)
    }
}

fn absence_error() -> AdapterError {
    AdapterError::surface_absent(crate::SnapshotSurface::Menu, "No open menu")
}

fn menu_source() -> RefEntry {
    let mut entry = menu_entry();
    entry.source.source_surface = crate::SnapshotSurface::Menu;
    entry
}

fn adapter() -> SurfaceAbsenceAdapter {
    SurfaceAbsenceAdapter {
        inner: windowless_adapter(),
        error: absence_error(),
        fail_after_observations: 0,
    }
}

fn wait(adapter: &SurfaceAbsenceAdapter, gone: bool) -> Result<Value, AppError> {
    let context = crate::CommandContext::default().with_wait_selector(Some(wait_for_ready(gone)));
    super::super::super::apply_post_action_wait(
        serde_json::to_value(crate::ActionResult::delivered_unverified("click")).unwrap(),
        Some(&menu_source()),
        adapter,
        &context,
    )
}

#[test]
fn complete_menu_absence_satisfies_only_gone_for_application_owner() {
    let adapter = adapter();
    assert_eq!(wait(&adapter, true).unwrap()["target_absent"], true);
    assert_eq!(wait(&adapter, false).unwrap_err().code(), "TIMEOUT");
    let result = observe(&wait_for_ready(true), &menu_source(), &adapter);
    assert_eq!(result["outcome"], "expected_state_observed");
    assert_eq!(result["selector_matches"], false);
    assert_eq!(result["surface"], "menu");
}

#[test]
fn incomplete_or_different_surface_errors_do_not_prove_menu_absence() {
    for details in [
        json!({}),
        json!({"kind": "surface_absent", "surface": "menu", "complete": false}),
        json!({"kind": "surface_absent", "surface": "menu"}),
        json!({"kind": "surface_absent", "surface": "menubar", "complete": true}),
    ] {
        let adapter = SurfaceAbsenceAdapter {
            error: AdapterError::new(ErrorCode::ElementNotFound, "Menu read failed")
                .with_details(details),
            ..adapter()
        };
        assert_eq!(
            wait(&adapter, true).unwrap_err().code(),
            "ELEMENT_NOT_FOUND"
        );
        let result = observe(&wait_for_ready(true), &menu_source(), &adapter);
        assert_eq!(result["outcome"], "observation_failed");
        assert!(result.get("selector_matches").is_none());
    }
}

#[test]
fn menu_dismissal_between_poll_and_materialization_cannot_satisfy_presence() {
    let adapter = SurfaceAbsenceAdapter {
        fail_after_observations: 1,
        ..adapter()
    };
    let error = wait(&adapter, false).unwrap_err();
    let AppError::Adapter(error) = error else {
        panic!("expected adapter timeout")
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::delivered_unverified()
    );
    assert!(error.details.unwrap().get("snapshot_id").is_none());
    assert_eq!(adapter.inner.observed.lock().unwrap().len(), 1);
}

#[test]
fn transient_observation_exhaustion_reports_timeout_and_keeps_last_error() {
    for error in [
        AdapterError::timeout("Read budget exhausted"),
        AdapterError::app_unresponsive("Owner").with_details(json!({"retryable": true})),
    ] {
        let adapter = SurfaceAbsenceAdapter { error, ..adapter() };
        let result = observe(&wait_for_ready(true), &menu_source(), &adapter);
        assert_eq!(result["outcome"], "timed_out");
        assert_eq!(
            result["last_observation_error"]["code"],
            adapter.error.code.as_str()
        );
        assert!(result.get("selector_matches").is_none());
    }
}
