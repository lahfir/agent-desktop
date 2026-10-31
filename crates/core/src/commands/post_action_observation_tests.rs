use super::*;
use crate::{
    AccessibilityNode, DeliverySemantics, ErrorCode, ProcessIdentity, WindowInfo,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree},
};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};

struct ProbeAdapter {
    windows: Vec<WindowInfo>,
    matching_pid: Option<crate::ProcessId>,
    observed: Mutex<Vec<(crate::ProcessId, crate::SnapshotSurface)>>,
    process_state: ProcessState,
    exits_after_observation: bool,
    incomplete: bool,
    process_checks: AtomicUsize,
    inventory_queries: AtomicUsize,
}

impl ObservationOps for ProbeAdapter {
    fn is_application_surface(&self, surface: crate::SnapshotSurface) -> bool {
        surface == crate::SnapshotSurface::Menubar
    }

    fn query_app_inventory(
        &self,
        _query: crate::AppInventoryQuery<'_>,
        _deadline: Deadline,
    ) -> Result<crate::AppInventory, AdapterError> {
        self.inventory_queries.fetch_add(1, Ordering::SeqCst);
        Err(AdapterError::new(
            ErrorCode::PermDenied,
            "Unrelated process denies application inventory",
        ))
    }

    fn list_windows(
        &self,
        _filter: &crate::WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::PermDenied,
            "Global window listing denied",
        ))
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let pid = match root {
            ObservationRoot::Application { app, .. } => {
                assert_eq!(app.pid, crate::ProcessId::new(1));
                assert_eq!(app.process_instance.as_deref(), Some("test-instance"));
                app.pid
            }
            ObservationRoot::Window(window) => {
                if request.surface != crate::SnapshotSurface::Window {
                    return Err(AdapterError::new(
                        ErrorCode::ElementNotFound,
                        "No open child surface",
                    ));
                }
                window.pid
            }
            ObservationRoot::Element { .. } => {
                return Err(AdapterError::internal("unexpected element root"));
            }
        };
        self.observed.lock().unwrap().push((pid, request.surface));
        let children = (self.matching_pid == Some(pid))
            .then(|| AccessibilityNode {
                ref_id: None,
                role: "button".into(),
                identity: crate::NodeIdentity {
                    name: Some("Ready".into()),
                    ..Default::default()
                },
                presentation: Default::default(),
                children_count: None,
                subtree_truncated: false,
                children: vec![],
            })
            .into_iter()
            .collect();
        crate::adapter::observed_tree(
            &root,
            AccessibilityNode {
                ref_id: None,
                role: "application".into(),
                identity: Default::default(),
                presentation: Default::default(),
                children_count: None,
                subtree_truncated: self.incomplete,
                children,
            },
        )
    }
}

impl ActionOps for ProbeAdapter {}
impl InputOps for ProbeAdapter {}
impl SystemOps for ProbeAdapter {
    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        assert_eq!(expected.id, "source-window");
        assert_eq!(expected.pid, crate::ProcessId::new(1));
        assert_eq!(expected.process_instance.as_deref(), Some("test-instance"));
        let candidate = self
            .windows
            .iter()
            .find(|window| window.id == expected.id && window.pid == expected.pid);
        if candidate.is_some_and(|window| window.process_instance.is_none()) {
            return Err(AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Incomplete window process identity",
            ));
        }
        candidate
            .filter(|window| window.process_instance == expected.process_instance)
            .cloned()
            .ok_or_else(|| {
                AdapterError::new(ErrorCode::WindowNotFound, "Original window is absent")
            })
    }

    fn process_state(
        &self,
        process: ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<ProcessState, AdapterError> {
        assert_eq!(
            process,
            ProcessIdentity::new(crate::ProcessId::new(1), "test-instance")
        );
        self.process_checks.fetch_add(1, Ordering::SeqCst);
        if self.exits_after_observation && !self.observed.lock().unwrap().is_empty() {
            return Ok(ProcessState::Exited { code: None });
        }
        Ok(self.process_state)
    }
}

fn window(pid: u32, instance: &str) -> WindowInfo {
    WindowInfo {
        app: "TargetApp".into(),
        pid: crate::ProcessId::new(pid),
        process_instance: Some(instance.into()),
        id: "source-window".into(),
        title: "Target window".into(),
        bounds: None,
        state: Default::default(),
    }
}

fn entry() -> RefEntry {
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.source.source_app = Some("TargetApp".into());
    entry.source.source_surface = crate::SnapshotSurface::Window;
    entry.source.source_window_id = Some("source-window".into());
    entry
}

fn wait_for_ready(gone: bool) -> WaitSelector {
    WaitSelector {
        query_raw: ":Ready".into(),
        gone,
        timeout_ms: 250,
    }
}

#[test]
fn selector_observation_uses_the_pinned_process_and_post_action_window_scope() {
    let adapter = ProbeAdapter {
        windows: vec![window(1, "test-instance")],
        matching_pid: Some(crate::ProcessId::new(1)),
        observed: Mutex::new(Vec::new()),
        process_state: ProcessState::Running,
        exits_after_observation: false,
        incomplete: false,
        process_checks: AtomicUsize::new(0),
        inventory_queries: AtomicUsize::new(0),
    };
    let result = observe(&wait_for_ready(false), &entry(), &adapter);

    assert_eq!(result["outcome"], "expected_state_observed");
    assert_eq!(result["selector_matches"], true);
    assert_eq!(result["surface"], "window");
    assert_eq!(
        *adapter.observed.lock().unwrap(),
        vec![(crate::ProcessId::new(1), crate::SnapshotSurface::Window)]
    );
}

#[test]
fn attaching_observation_preserves_the_original_action_error() {
    let error = AdapterError::new(ErrorCode::AppUnresponsive, "AXPress timed out")
        .with_suggestion("Inspect state")
        .with_platform_detail("kAXErrorCannotComplete")
        .with_details(json!({ "ax_error": -25204 }))
        .with_disposition(DeliverySemantics::uncertain());
    let original = error.clone();
    let attached = attach(error, json!({ "outcome": "expected_state_observed" }));

    assert_eq!(attached.code, original.code);
    assert_eq!(attached.message, original.message);
    assert_eq!(attached.suggestion, original.suggestion);
    assert_eq!(attached.platform_detail, original.platform_detail);
    assert_eq!(attached.disposition, original.disposition);
    let details = attached.details.unwrap();
    assert_eq!(details["ax_error"], -25204);
    assert_eq!(
        details["post_action_observation"]["outcome"],
        "expected_state_observed"
    );
}

fn windowless_adapter() -> ProbeAdapter {
    ProbeAdapter {
        windows: vec![],
        matching_pid: Some(crate::ProcessId::new(1)),
        observed: Mutex::new(Vec::new()),
        process_state: ProcessState::Running,
        exits_after_observation: false,
        incomplete: false,
        process_checks: AtomicUsize::new(0),
        inventory_queries: AtomicUsize::new(0),
    }
}

fn menu_entry() -> RefEntry {
    let mut entry = entry();
    entry.source.source_surface = crate::SnapshotSurface::Menubar;
    entry.source.source_window_id = None;
    entry
}

fn delivered_wait(adapter: &ProbeAdapter, entry: &RefEntry, gone: bool) -> Result<Value, AppError> {
    let context = crate::CommandContext::default().with_wait_selector(Some(wait_for_ready(gone)));
    super::super::apply_post_action_wait(
        serde_json::to_value(crate::ActionResult::delivered_unverified("click")).unwrap(),
        Some(entry),
        adapter,
        &context,
    )
}

#[test]
fn windowless_menubar_waits_read_application_for_presence_and_absence() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    for gone in [false, true] {
        let mut adapter = windowless_adapter();
        adapter.matching_pid = (!gone).then_some(crate::ProcessId::new(1));
        let mut entry = menu_entry();
        entry.source.source_app = Some("Saved label".into());
        let reconciled = observe(&wait_for_ready(gone), &entry, &adapter);
        assert_eq!(reconciled["outcome"], "expected_state_observed");
        assert_eq!(reconciled["surface"], "menubar");
        assert_eq!(reconciled["selector_matches"], !gone);
        let result = delivered_wait(&adapter, &entry, gone).unwrap();
        if gone {
            assert_eq!(result["target_absent"], true);
        } else {
            assert_eq!(result["app"], "Saved label");
        }
        assert_eq!(adapter.inventory_queries.load(Ordering::SeqCst), 0);
        assert!(
            adapter
                .observed
                .lock()
                .unwrap()
                .iter()
                .all(|(pid, surface)| *pid == crate::ProcessId::new(1)
                    && *surface == crate::SnapshotSurface::Menubar)
        );
    }
}

#[test]
fn successful_menu_materialization_rechecks_original_process_liveness() {
    let adapter = ProbeAdapter {
        exits_after_observation: true,
        ..windowless_adapter()
    };
    let error = delivered_wait(&adapter, &menu_entry(), false).unwrap_err();
    assert_eq!(error.code(), "TIMEOUT");
    assert_eq!(adapter.observed.lock().unwrap().len(), 1);
    assert!(adapter.process_checks.load(Ordering::SeqCst) >= 2);
}

#[test]
fn original_window_is_required_even_when_same_process_has_another_window() {
    let mut adapter = windowless_adapter();
    let mut other = window(1, "test-instance");
    other.id = "other-window".into();
    adapter.windows = vec![other];
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
    let result = observe(&wait_for_ready(true), &entry(), &adapter);
    assert_eq!(result["outcome"], "expected_state_observed");
    assert_eq!(result["selector_matches"], false);
    assert!(adapter.observed.lock().unwrap().is_empty());
}

#[test]
fn present_menu_times_out_gone_wait_with_source_surface_diagnostic() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let adapter = windowless_adapter();
    let error = delivered_wait(&adapter, &menu_entry(), true).unwrap_err();
    let AppError::Adapter(error) = error else {
        panic!("expected adapter timeout")
    };
    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.disposition,
        crate::DeliverySemantics::delivered_unverified()
    );
    let details = error.details.unwrap();
    let snapshot_id = details["snapshot_id"].as_str().unwrap();
    let refmap = crate::refs::refs_store::RefStore::new()
        .unwrap()
        .load_snapshot(snapshot_id)
        .unwrap();
    let saved = refmap.get("@e1").unwrap();
    assert_eq!(saved.source.source_surface, crate::SnapshotSurface::Menubar);
    assert!(saved.source.source_window_id.is_none());
    assert_eq!(
        saved.process.process_instance.as_deref(),
        Some("test-instance")
    );
    assert!(
        adapter
            .observed
            .lock()
            .unwrap()
            .iter()
            .all(|(_, surface)| *surface == crate::SnapshotSurface::Menubar)
    );
}

#[path = "post_action_observation_lifecycle_tests.rs"]
mod lifecycle_tests;

#[path = "post_action_observation_surface_tests.rs"]
mod surface_tests;

#[test]
fn sheet_ref_waits_observe_window_presence_and_confirm_absence() {
    let _home = crate::refs::refs_test_support::HomeGuard::new();
    let mut source = entry();
    source.source.source_surface = crate::SnapshotSurface::Sheet;
    let mut adapter = windowless_adapter();
    adapter.windows = vec![window(1, "test-instance")];
    assert!(
        delivered_wait(&adapter, &source, false)
            .unwrap()
            .get("snapshot_id")
            .is_some()
    );
    assert_eq!(
        delivered_wait(&adapter, &source, true).unwrap_err().code(),
        "TIMEOUT"
    );
    adapter.matching_pid = None;
    assert_eq!(
        delivered_wait(&adapter, &source, true).unwrap()["target_absent"],
        true
    );
    let result = observe(&wait_for_ready(true), &source, &adapter);
    assert_eq!(result["outcome"], "expected_state_observed");
    assert_eq!(result["surface"], "window");
    assert!(
        adapter
            .observed
            .lock()
            .unwrap()
            .iter()
            .all(|(_, surface)| *surface == crate::SnapshotSurface::Window)
    );
}
