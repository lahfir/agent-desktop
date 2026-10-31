use super::*;

#[test]
fn unresponsive_original_process_does_not_prove_selector_absence() {
    let mut adapter = windowless_adapter();
    adapter.process_state = ProcessState::Unresponsive;
    for gone in [false, true] {
        assert_eq!(
            delivered_wait(&adapter, &menu_entry(), gone)
                .unwrap_err()
                .code(),
            "TIMEOUT"
        );
        let result = observe(&wait_for_ready(gone), &menu_entry(), &adapter);
        assert_eq!(result["outcome"], "owner_unavailable");
        assert_eq!(result["process_state"], "unresponsive");
        assert!(result.get("selector_matches").is_none());
    }
    assert!(adapter.process_checks.load(Ordering::SeqCst) > 4);
    assert_eq!(adapter.inventory_queries.load(Ordering::SeqCst), 0);
    assert!(adapter.observed.lock().unwrap().is_empty());
}

#[test]
fn exited_original_application_satisfies_only_a_gone_wait() {
    for state in [
        ProcessState::Exited { code: None },
        ProcessState::Crashed { signal_or_code: 9 },
    ] {
        let mut adapter = windowless_adapter();
        adapter.process_state = state;
        assert_eq!(
            delivered_wait(&adapter, &menu_entry(), true).unwrap()["target_absent"],
            true
        );
        assert_eq!(
            delivered_wait(&adapter, &menu_entry(), false)
                .unwrap_err()
                .code(),
            "TIMEOUT"
        );
        let result = observe(&wait_for_ready(true), &menu_entry(), &adapter);
        assert_eq!(
            result["outcome"],
            if matches!(state, ProcessState::Crashed { .. }) {
                "process_crashed"
            } else {
                "process_exited"
            }
        );
        assert!(result.get("selector_matches").is_none());
        assert!(adapter.observed.lock().unwrap().is_empty());
    }
}

#[test]
fn replacement_window_cannot_satisfy_positive_wait_for_original_generation() {
    let mut adapter = windowless_adapter();
    adapter.windows = vec![window(1, "replacement")];
    assert_eq!(
        delivered_wait(&adapter, &entry(), false)
            .unwrap_err()
            .code(),
        "TIMEOUT"
    );
    assert_eq!(
        delivered_wait(&adapter, &entry(), true).unwrap()["target_absent"],
        true
    );
    assert!(adapter.observed.lock().unwrap().is_empty());
}

#[test]
fn incomplete_window_identity_cannot_prove_original_window_absence() {
    let mut adapter = windowless_adapter();
    let mut original = window(1, "test-instance");
    original.process_instance = None;
    adapter.windows = vec![original];
    assert_eq!(
        delivered_wait(&adapter, &entry(), true).unwrap_err().code(),
        "ACTION_NOT_SUPPORTED"
    );
    assert!(adapter.observed.lock().unwrap().is_empty());
}

#[test]
fn application_owner_requires_saved_label_and_nonempty_generation() {
    let adapter = windowless_adapter();
    let mut missing_label = menu_entry();
    missing_label.source.source_app = None;
    assert_eq!(
        delivered_wait(&adapter, &missing_label, true)
            .unwrap_err()
            .code(),
        "STALE_REF"
    );
    for generation in [None, Some(String::new())] {
        let mut entry = menu_entry();
        entry.process.process_instance = generation;
        assert_eq!(
            delivered_wait(&adapter, &entry, true).unwrap_err().code(),
            "ACTION_NOT_SUPPORTED"
        );
    }
    assert_eq!(adapter.process_checks.load(Ordering::SeqCst), 1);
    assert_eq!(adapter.inventory_queries.load(Ordering::SeqCst), 0);
}

struct StrictReadAdapter {
    deadline: Deadline,
    code: ErrorCode,
}

impl ObservationOps for StrictReadAdapter {
    fn is_application_surface(&self, surface: crate::SnapshotSurface) -> bool {
        surface == crate::SnapshotSurface::Menubar
    }
}
impl ActionOps for StrictReadAdapter {}
impl InputOps for StrictReadAdapter {}
impl SystemOps for StrictReadAdapter {
    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        assert_eq!(deadline, self.deadline);
        assert_eq!(expected.id, "source-window");
        assert_eq!(expected.pid, crate::ProcessId::new(1));
        assert_eq!(expected.process_instance.as_deref(), Some("test-instance"));
        assert_eq!(expected.app, "Saved label");
        assert_eq!(expected.title, "Saved title");
        Err(
            AdapterError::new(self.code.clone(), "Exact window read failed")
                .with_details(json!({"original": true})),
        )
    }

    fn process_state(
        &self,
        process: ProcessIdentity,
        deadline: Deadline,
    ) -> Result<ProcessState, AdapterError> {
        assert_eq!(deadline, self.deadline);
        assert_eq!(
            process,
            ProcessIdentity::new(crate::ProcessId::new(1), "test-instance")
        );
        if self.code == ErrorCode::WindowNotFound {
            return Ok(ProcessState::Running);
        }
        Err(
            AdapterError::new(self.code.clone(), "Exact process read failed")
                .with_details(json!({"original": true})),
        )
    }
}

#[test]
fn strict_window_lookup_uses_saved_identity_and_only_not_found_proves_absence() {
    use crate::snapshot::{
        ref_owner_observation::RefOwnerObservation, surface_target::resolve_ref_owner,
    };
    let mut entry = entry();
    entry.source.source_app = Some("Saved label".into());
    entry.source.source_window_title = Some("Saved title".into());
    for code in [
        ErrorCode::WindowNotFound,
        ErrorCode::Timeout,
        ErrorCode::AppUnresponsive,
        ErrorCode::PermDenied,
        ErrorCode::ActionNotSupported,
        ErrorCode::StaleRef,
    ] {
        let adapter = StrictReadAdapter {
            deadline: Deadline::after(250).unwrap(),
            code: code.clone(),
        };
        let result = resolve_ref_owner(&adapter, &entry, adapter.deadline);
        if code == ErrorCode::WindowNotFound {
            assert!(matches!(result, Ok(RefOwnerObservation::Absent(None))));
        } else {
            let AppError::Adapter(error) = result.err().unwrap() else {
                panic!("expected adapter error")
            };
            assert_eq!(error.code, code);
            assert_eq!(error.message, "Exact window read failed");
            assert_eq!(error.details.unwrap()["original"], true);
        }
    }
}

#[test]
fn process_liveness_errors_preserve_deadline_and_original_error() {
    use crate::snapshot::surface_target::resolve_ref_owner;
    let adapter = StrictReadAdapter {
        deadline: Deadline::after(250).unwrap(),
        code: ErrorCode::PermDenied,
    };
    let error = resolve_ref_owner(&adapter, &menu_entry(), adapter.deadline)
        .err()
        .unwrap();
    let AppError::Adapter(error) = error else {
        panic!("expected adapter error")
    };
    assert_eq!(error.code, ErrorCode::PermDenied);
    assert_eq!(error.message, "Exact process read failed");
    assert_eq!(error.details.unwrap()["original"], true);
}

#[test]
fn absent_window_reconciliation_preserves_original_process_exit_and_crash() {
    for state in [
        ProcessState::Exited { code: None },
        ProcessState::Crashed { signal_or_code: 9 },
    ] {
        let mut adapter = windowless_adapter();
        adapter.process_state = state;
        for gone in [false, true] {
            let result = observe(&wait_for_ready(gone), &entry(), &adapter);
            assert_eq!(
                result["outcome"],
                if matches!(state, ProcessState::Crashed { .. }) {
                    "process_crashed"
                } else {
                    "process_exited"
                }
            );
            assert_eq!(result["process_state"], state.label());
            assert!(result.get("selector_matches").is_none());
        }
        assert_eq!(
            delivered_wait(&adapter, &entry(), true).unwrap()["target_absent"],
            true
        );
        assert_eq!(
            delivered_wait(&adapter, &entry(), false)
                .unwrap_err()
                .code(),
            "TIMEOUT"
        );
        assert!(adapter.observed.lock().unwrap().is_empty());
        assert_eq!(adapter.inventory_queries.load(Ordering::SeqCst), 0);
    }
}
