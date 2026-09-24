//! `--wait-for` after a background pointer event must observe the target
//! window, not whatever app the user has frontmost.
use super::test_support::{PID, WINDOW_ID, left_click, point_args, window_bounds};
use super::*;
use crate::adapter::{
    ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, TreeOptions, WindowFilter,
};
use crate::context::WaitSelector;
use crate::live_locator::{ObservationRequest, ObservedTree};
use crate::refs_test_support::HomeGuard;
use crate::{AccessibilityNode, ProcessId};
use std::sync::Mutex;

const FRONTMOST_WINDOW_ID: &str = "w-1";

/// The user's focused window belongs to another app and never shows the
/// confirmation; only the background target window does.
struct FrontmostElsewhereAdapter {
    observed: Mutex<Vec<String>>,
}

impl FrontmostElsewhereAdapter {
    fn windows() -> Vec<WindowInfo> {
        vec![
            WindowInfo {
                id: FRONTMOST_WINDOW_ID.into(),
                title: "User".into(),
                app: "Finder".into(),
                pid: ProcessId::new(7),
                process_instance: Some("test-instance".into()),
                bounds: None,
                state: WindowState {
                    is_focused: true,
                    ..WindowState::default()
                },
            },
            WindowInfo {
                id: WINDOW_ID.into(),
                title: "Target".into(),
                app: "Code".into(),
                pid: ProcessId::new(PID),
                process_instance: Some("test-instance".into()),
                bounds: Some(window_bounds()),
                state: WindowState::default(),
            },
        ]
    }

    fn window_node(window: &WindowInfo) -> AccessibilityNode {
        let label = if window.id == WINDOW_ID {
            "Saved"
        } else {
            "Unrelated"
        };
        let child = AccessibilityNode {
            ref_id: None,
            role: "button".into(),
            identity: crate::NodeIdentity {
                retained_object: None,
                name: Some(label.into()),
                ..Default::default()
            },
            presentation: Default::default(),
            children_count: None,
            subtree_truncated: false,
            children: vec![],
        };
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
            children: vec![child],
        }
    }
}

impl ObservationOps for FrontmostElsewhereAdapter {
    fn observe_tree(
        &self,
        root: crate::live_locator::ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        let crate::live_locator::ObservationRoot::Window(window) = root else {
            return Err(AdapterError::internal("expected window root"));
        };
        self.observed.lock().unwrap().push(window.id.clone());
        let node = Self::window_node(window);
        crate::adapter::observed_tree(&crate::live_locator::ObservationRoot::Window(window), node)
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
        Ok(Self::windows()
            .into_iter()
            .filter(|window| filter.app.as_deref().is_none_or(|app| window.app == app))
            .collect())
    }

    fn get_tree(
        &self,
        window: &WindowInfo,
        _opts: &TreeOptions,
        _deadline: crate::Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        Ok(Self::window_node(window))
    }

    crate::adapter::complete_live_observation!("button", "Saved", [crate::capability::CLICK]);
}

impl ActionOps for FrontmostElsewhereAdapter {}

impl InputOps for FrontmostElsewhereAdapter {
    fn background_mouse_event(
        &self,
        _window: &WindowInfo,
        _event: MouseEvent,
        _lease: &crate::InteractionLease,
    ) -> Result<BackgroundPointerReport, AdapterError> {
        Ok(BackgroundPointerReport::default())
    }
}

impl SystemOps for FrontmostElsewhereAdapter {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();
}

#[test]
fn coordinate_click_waits_on_the_target_window_not_the_frontmost_app() {
    let _guard = HomeGuard::new();
    let adapter = FrontmostElsewhereAdapter {
        observed: Mutex::new(Vec::new()),
    };
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":saved".into(),
        gone: false,
        timeout_ms: 500,
    }));

    let value = execute(
        point_args(left_click(1), -2500.0, 300.0),
        &adapter,
        &context,
    )
    .expect("the confirmation exists only in the target window");

    assert_eq!(value["matched_selector"], ":saved");
    assert_eq!(value["after_action"]["clicked"], true);
    let observed = adapter.observed.lock().unwrap();
    assert!(
        !observed.is_empty() && observed.iter().all(|id| id == WINDOW_ID),
        "only the target window may be observed: {observed:?}"
    );
}
