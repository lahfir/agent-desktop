use super::*;
use crate::{
    AccessibilityNode, DeliverySemantics, ErrorCode, WindowInfo,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    locator::live_locator::{ObservationRequest, ObservationRoot, ObservedTree},
};
use std::sync::Mutex;

struct ProbeAdapter {
    windows: Vec<WindowInfo>,
    matching_pid: Option<crate::ProcessId>,
    observed: Mutex<Vec<(crate::ProcessId, crate::SnapshotSurface)>>,
    process_state: ProcessState,
}

impl ObservationOps for ProbeAdapter {
    fn list_windows(
        &self,
        filter: &crate::WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        assert_eq!(filter.app.as_deref(), Some("TargetApp"));
        Ok(self.windows.clone())
    }

    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let pid = match root {
            ObservationRoot::Application { app, .. } => app.pid,
            ObservationRoot::Window(window) => window.pid,
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
                subtree_truncated: false,
                children,
            },
        )
    }
}

impl ActionOps for ProbeAdapter {}
impl InputOps for ProbeAdapter {}
impl SystemOps for ProbeAdapter {
    fn process_state(
        &self,
        _process: ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<ProcessState, AdapterError> {
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
    entry.source.source_surface = crate::SnapshotSurface::Menubar;
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
fn replacement_process_and_process_exit_never_satisfy_gone() {
    let replacement = ProbeAdapter {
        windows: vec![window(2, "new-instance")],
        matching_pid: Some(crate::ProcessId::new(2)),
        observed: Mutex::new(Vec::new()),
        process_state: ProcessState::Running,
    };
    let result = observe(&wait_for_ready(true), &entry(), &replacement);

    assert_eq!(result["outcome"], "window_unavailable");
    assert!(result.get("selector_matches").is_none());
    assert!(replacement.observed.lock().unwrap().is_empty());

    let exited = ProbeAdapter {
        windows: Vec::new(),
        matching_pid: None,
        observed: Mutex::new(Vec::new()),
        process_state: ProcessState::Exited { code: None },
    };
    let result = observe(&wait_for_ready(true), &entry(), &exited);

    assert_eq!(result["outcome"], "process_exited");
    assert!(result.get("selector_matches").is_none());
}

#[test]
fn selector_observation_uses_the_pinned_process_and_post_action_window_scope() {
    let adapter = ProbeAdapter {
        windows: vec![window(1, "test-instance")],
        matching_pid: Some(crate::ProcessId::new(1)),
        observed: Mutex::new(Vec::new()),
        process_state: ProcessState::Running,
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
