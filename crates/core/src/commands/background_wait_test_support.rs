//! An adapter whose frontmost window belongs to another app, for proving
//! that `--wait-for` after background delivery observes the target window.
use crate::adapter::{
    ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, TreeOptions, WindowFilter,
};
use crate::live_locator::{ObservationRequest, ObservedTree};
use crate::{
    AccessibilityNode, AdapterError, BackgroundDeliveryReport, MouseEvent, ProcessId, RefEntry,
    WindowInfo, WindowState,
};
use std::sync::Mutex;

const FRONTMOST_WINDOW_ID: &str = "w-1";

/// The user's focused window belongs to another app and never shows the
/// confirmation; only the background target window does. Every observed
/// window id is recorded so a test can prove the wait looked only at the
/// target.
pub(crate) struct FrontmostElsewhereAdapter {
    target: WindowInfo,
    pub(crate) observed: Mutex<Vec<String>>,
}

impl FrontmostElsewhereAdapter {
    pub(crate) fn new(target: WindowInfo) -> Self {
        Self {
            target,
            observed: Mutex::new(Vec::new()),
        }
    }

    fn windows(&self) -> Vec<WindowInfo> {
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
            self.target.clone(),
        ]
    }

    fn window_node(&self, window: &WindowInfo) -> AccessibilityNode {
        let label = if window.id == self.target.id {
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
        let node = self.window_node(window);
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
        Ok(self
            .windows()
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
        Ok(self.window_node(window))
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
    ) -> Result<BackgroundDeliveryReport, AdapterError> {
        Ok(BackgroundDeliveryReport::default())
    }

    fn background_key_input(
        &self,
        _window: &WindowInfo,
        _input: &crate::BackgroundKeyInput,
        _lease: &crate::InteractionLease,
    ) -> Result<BackgroundDeliveryReport, AdapterError> {
        Ok(BackgroundDeliveryReport::default())
    }
}

impl SystemOps for FrontmostElsewhereAdapter {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();
}
