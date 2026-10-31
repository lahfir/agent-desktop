use super::test_support::entry;
use super::*;
use crate::AdapterError;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter};
use crate::context::WaitSelector;
use crate::refs::RefMap;
use crate::refs::refs_test_support::HomeGuard;
use crate::{AccessibilityNode, WindowInfo};
use crate::{actions::action::Action, actions::action_result::ActionResult};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU32, Ordering},
};

struct LeaseGuard(Arc<AtomicBool>);

impl Drop for LeaseGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

struct ScopedWaitAdapter {
    request: Mutex<Option<ActionRequest>>,
    polled_app: Mutex<Option<String>>,
    lease_held: Arc<AtomicBool>,
    lease_free_polls: AtomicU32,
    dispatches: AtomicU32,
    uncertain: bool,
    closes_window: bool,
}

impl ScopedWaitAdapter {
    fn new() -> Self {
        Self {
            request: Mutex::new(None),
            polled_app: Mutex::new(None),
            lease_held: Arc::new(AtomicBool::new(false)),
            lease_free_polls: AtomicU32::new(0),
            dispatches: AtomicU32::new(0),
            uncertain: false,
            closes_window: false,
        }
    }
}

impl ObservationOps for ScopedWaitAdapter {
    fn is_application_surface(&self, surface: crate::SnapshotSurface) -> bool {
        surface == crate::SnapshotSurface::Menubar
    }

    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        if request.surface == crate::SnapshotSurface::Menubar {
            let crate::locator::live_locator::ObservationRoot::Application { app, .. } = root
            else {
                return Err(AdapterError::internal("expected application root"));
            };
            *self.polled_app.lock().unwrap() = Some(app.name.clone());
        }
        if self.lease_held.load(Ordering::SeqCst) {
            return Err(AdapterError::internal(
                "post-action wait must poll with the interaction lease released",
            ));
        }
        self.lease_free_polls.fetch_add(1, Ordering::SeqCst);
        crate::adapter::observed_tree(
            &root,
            AccessibilityNode {
                ref_id: None,
                role: "window".into(),
                identity: crate::NodeIdentity {
                    retained_object: None,
                    name: Some("Saved!".into()),
                    ..Default::default()
                },
                presentation: Default::default(),
                children_count: None,
                subtree_truncated: false,
                children: vec![],
            },
        )
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn list_windows(
        &self,
        filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        *self.polled_app.lock().unwrap() = filter.app.clone();
        if self.closes_window && self.dispatches.load(Ordering::SeqCst) > 0 {
            return Ok(Vec::new());
        }
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: filter.app.clone().unwrap_or_else(|| "TargetApp".into()),
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }

    crate::adapter::complete_live_observation!("button", "OK", [crate::actions::capability::CLICK]);
}

impl ActionOps for ScopedWaitAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        assert!(self.lease_held.load(Ordering::SeqCst));
        *self.request.lock().unwrap() = Some(request);
        self.dispatches.fetch_add(1, Ordering::SeqCst);
        if self.uncertain {
            return Err(AdapterError::app_unresponsive("TargetApp")
                .with_details(json!({ "ax_error": -25204 }))
                .with_disposition(crate::DeliverySemantics::uncertain()));
        }
        Ok(ActionResult::delivered_unverified("ok"))
    }
}

impl InputOps for ScopedWaitAdapter {}

impl SystemOps for ScopedWaitAdapter {
    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        assert_eq!(expected.id, "w-1");
        assert_eq!(expected.pid, crate::ProcessId::new(1));
        assert_eq!(expected.process_instance.as_deref(), Some("test-instance"));
        if self.closes_window && self.dispatches.load(Ordering::SeqCst) > 0 {
            return Err(AdapterError::new(
                crate::ErrorCode::WindowNotFound,
                "Original window closed",
            ));
        }
        Ok(expected.clone())
    }

    fn process_state(
        &self,
        process: crate::ProcessIdentity,
        _deadline: crate::Deadline,
    ) -> Result<crate::system::process_state::ProcessState, AdapterError> {
        assert_eq!(
            process,
            crate::ProcessIdentity::new(crate::ProcessId::new(1), "test-instance")
        );
        Ok(crate::system::process_state::ProcessState::Running)
    }

    fn acquire_interaction_lease(
        &self,
        deadline: crate::Deadline,
    ) -> Result<crate::InteractionLease, AdapterError> {
        self.lease_held
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| AdapterError::internal("test lease already held"))?;
        crate::InteractionLease::guarded(deadline, LeaseGuard(Arc::clone(&self.lease_held)))
    }
}

#[test]
fn post_action_wait_scopes_and_reconciles_without_redispatch_or_held_lease() {
    let _guard = HomeGuard::new();
    for uncertain in [false, true] {
        let mut refmap = RefMap::new();
        let mut entry = entry();
        entry.source.source_app = Some("TargetApp".into());
        entry.source.source_surface = crate::SnapshotSurface::Menubar;
        assert!(entry.source.source_window_id.is_none());
        refmap.allocate(entry);
        let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
        let mut adapter = ScopedWaitAdapter::new();
        adapter.uncertain = uncertain;
        let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
            query_raw: ":saved!".into(),
            gone: false,
            timeout_ms: 500,
        }));
        let result = execute_ref_action_with_context(
            RefArgs {
                ref_id: format!("@{snapshot_id}:e1"),
                timeout_ms: None,
            },
            &adapter,
            ActionRequest::headless(Action::Click),
            &context,
        );
        assert_eq!(adapter.dispatches.load(Ordering::SeqCst), 1);
        assert_eq!(
            adapter.polled_app.lock().unwrap().as_deref(),
            Some("TargetApp")
        );
        assert!(!adapter.lease_held.load(Ordering::SeqCst));
        assert!(adapter.lease_free_polls.load(Ordering::SeqCst) >= 1);
        if uncertain {
            let AppError::Adapter(error) = result.unwrap_err() else {
                panic!("expected adapter error")
            };
            assert_eq!(error.code, crate::ErrorCode::AppUnresponsive);
            assert_eq!(error.disposition, crate::DeliverySemantics::uncertain());
            let details = error.details.unwrap();
            assert_eq!(details["ax_error"], -25204);
            assert_eq!(
                details["post_action_observation"]["outcome"],
                "expected_state_observed"
            );
        } else {
            let value = result.unwrap();
            assert_eq!(value["after_action"]["action"], "ok");
            assert_eq!(value["matched_selector"], ":saved!");
        }
    }
}

struct MultiWindowAdapter;

impl ObservationOps for MultiWindowAdapter {
    fn observe_tree(
        &self,
        root: crate::locator::live_locator::ObservationRoot<'_>,
        _request: &crate::locator::live_locator::ObservationRequest,
    ) -> Result<crate::locator::live_locator::ObservedTree, AdapterError> {
        let crate::locator::live_locator::ObservationRoot::Window(window) = root else {
            return Err(AdapterError::internal("expected window root"));
        };
        let children = (window.id == "w-target")
            .then(|| AccessibilityNode {
                ref_id: None,
                role: "button".into(),
                identity: crate::NodeIdentity {
                    retained_object: None,
                    name: Some("Saved!".into()),
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
            &crate::locator::live_locator::ObservationRoot::Window(window),
            AccessibilityNode {
                ref_id: None,
                role: "window".into(),
                identity: crate::NodeIdentity {
                    retained_object: None,
                    name: Some(window.title.clone()),
                    ..Default::default()
                },
                presentation: Default::default(),
                children_count: None,
                subtree_truncated: false,
                children,
            },
        )
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![
            WindowInfo {
                id: "w-other".into(),
                title: "Other".into(),
                app: "App".into(),
                pid: crate::ProcessId::new(1),
                process_instance: Some("test-instance".into()),
                bounds: None,
                state: crate::WindowState {
                    is_focused: true,
                    ..Default::default()
                },
            },
            WindowInfo {
                id: "w-target".into(),
                title: "Target".into(),
                app: "App".into(),
                pid: crate::ProcessId::new(1),
                process_instance: Some("test-instance".into()),
                bounds: None,
                state: crate::WindowState {
                    is_focused: false,
                    ..Default::default()
                },
            },
        ])
    }

    crate::adapter::complete_live_observation!("button", "OK", [crate::actions::capability::CLICK]);
}

impl ActionOps for MultiWindowAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::delivered_unverified("ok"))
    }
}

impl InputOps for MultiWindowAdapter {}

impl SystemOps for MultiWindowAdapter {
    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        assert_eq!(expected.id, "w-target");
        assert_eq!(expected.pid, crate::ProcessId::new(1));
        assert_eq!(expected.process_instance.as_deref(), Some("test-instance"));
        Ok(expected.clone())
    }

    crate::adapter::guarded_interaction_lease!();
}

#[test]
fn post_action_wait_polls_acted_on_window_not_focused_window() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    let mut entry = entry();
    entry.source.source_app = Some("App".into());
    entry.source.source_window_id = Some("w-target".into());
    refmap.allocate(entry);
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":saved!".into(),
        gone: false,
        timeout_ms: 500,
    }));
    let args = RefArgs {
        ref_id: format!("@{}:e1", snapshot_id),
        timeout_ms: None,
    };

    let value = execute_ref_action_with_context(
        args,
        &MultiWindowAdapter,
        ActionRequest::headless(Action::Click),
        &context,
    )
    .expect("wait must match in the acted-on window, not the focused empty window");
    assert_eq!(value["matched_selector"], ":saved!");
    assert_eq!(value["window"]["id"], "w-target");
}

#[path = "helpers_ref_action_wait_result_tests.rs"]
mod result_tests;
