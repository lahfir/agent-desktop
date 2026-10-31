use super::*;

struct SurfaceAbsenceAdapter {
    inner: ProbeAdapter,
    error: AdapterError,
    application_owned: bool,
    fail_after_observations: usize,
}

impl ObservationOps for SurfaceAbsenceAdapter {
    fn is_application_surface(&self, surface: crate::SnapshotSurface) -> bool {
        self.application_owned && surface == crate::SnapshotSurface::Menu
    }

    fn query_app_inventory(
        &self,
        query: crate::AppInventoryQuery<'_>,
        deadline: Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        self.inner.query_app_inventory(query, deadline)
    }

    fn list_windows(
        &self,
        filter: &crate::WindowFilter,
        deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        self.inner.list_windows(filter, deadline)
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
    AdapterError::new(ErrorCode::ElementNotFound, "No open menu").with_details(json!({
        "kind": "surface_absent", "complete": true, "surface": "menu",
    }))
}

fn menu_source(application_owned: bool) -> RefEntry {
    let mut entry = if application_owned {
        menu_entry()
    } else {
        entry()
    };
    entry.source.source_surface = crate::SnapshotSurface::Menu;
    entry
}

fn adapter(application_owned: bool) -> SurfaceAbsenceAdapter {
    let mut inner = windowless_adapter();
    inner.windows = vec![window(1, "test-instance")];
    SurfaceAbsenceAdapter {
        inner,
        error: absence_error(),
        application_owned,
        fail_after_observations: 0,
    }
}

fn wait(adapter: &SurfaceAbsenceAdapter, gone: bool) -> Result<Value, AppError> {
    let context = crate::CommandContext::default().with_wait_selector(Some(wait_for_ready(gone)));
    super::super::super::apply_post_action_wait(
        serde_json::to_value(crate::ActionResult::delivered_unverified("click")).unwrap(),
        Some(&menu_source(adapter.application_owned)),
        adapter,
        &context,
    )
}

#[test]
fn complete_menu_absence_satisfies_only_gone_for_application_and_window_owners() {
    for application_owned in [true, false] {
        let adapter = adapter(application_owned);
        assert_eq!(wait(&adapter, true).unwrap()["target_absent"], true);
        assert_eq!(wait(&adapter, false).unwrap_err().code(), "TIMEOUT");
        let result = observe(
            &wait_for_ready(true),
            &menu_source(application_owned),
            &adapter,
        );
        assert_eq!(result["outcome"], "expected_state_observed");
        assert_eq!(result["selector_matches"], false);
        assert_eq!(result["surface"], "menu");
    }
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
            ..adapter(true)
        };
        assert_eq!(
            wait(&adapter, true).unwrap_err().code(),
            "ELEMENT_NOT_FOUND"
        );
        let result = observe(&wait_for_ready(true), &menu_source(true), &adapter);
        assert_eq!(result["outcome"], "observation_failed");
        assert!(result.get("selector_matches").is_none());
    }
}

#[test]
fn menu_dismissal_between_poll_and_materialization_cannot_satisfy_presence() {
    let adapter = SurfaceAbsenceAdapter {
        fail_after_observations: 1,
        ..adapter(true)
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
