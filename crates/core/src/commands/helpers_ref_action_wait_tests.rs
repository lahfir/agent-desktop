use super::test_support::entry;
use super::*;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps, WindowFilter};
use crate::context::WaitSelector;
use crate::error::AdapterError;
use crate::node::{AccessibilityNode, WindowInfo};
use crate::refs::RefMap;
use crate::refs_test_support::HomeGuard;
use crate::{action::Action, action_result::ActionResult};
use std::sync::Mutex;

struct ScopedWaitAdapter {
    request: Mutex<Option<ActionRequest>>,
    polled_app: Mutex<Option<String>>,
}

impl ObservationOps for ScopedWaitAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        *self.polled_app.lock().unwrap() = filter.app.clone();
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: filter.app.clone().unwrap_or_else(|| "TargetApp".into()),
            pid: 1,
            bounds: None,
            is_focused: true,
        }])
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &crate::adapter::TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        Ok(AccessibilityNode {
            ref_id: None,
            role: "window".into(),
            name: Some("Saved!".into()),
            value: None,
            description: None,
            native_id: None,
            hint: None,
            states: vec![],
            available_actions: vec![],
            bounds: None,
            children_count: None,
            children: vec![],
        })
    }
}

impl ActionOps for ScopedWaitAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        *self.request.lock().unwrap() = Some(request);
        Ok(ActionResult::new("ok"))
    }
}

impl InputOps for ScopedWaitAdapter {}

impl SystemOps for ScopedWaitAdapter {}

#[test]
fn post_action_wait_scopes_to_source_app_and_merges_action_result() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    let mut entry = entry();
    entry.source_app = Some("TargetApp".into());
    refmap.allocate(entry);
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = ScopedWaitAdapter {
        request: Mutex::new(None),
        polled_app: Mutex::new(None),
    };
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":saved!".into(),
        gone: false,
        timeout_ms: 5_000,
    }));
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
        timeout_ms: None,
    };

    let value = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &context,
    )
    .unwrap();

    assert_eq!(
        adapter.polled_app.lock().unwrap().as_deref(),
        Some("TargetApp")
    );
    assert_eq!(value["after_action"]["action"], "ok");
    assert_eq!(value["matched_selector"], ":saved!");
}

struct MultiWindowAdapter;

impl ObservationOps for MultiWindowAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![
            WindowInfo {
                id: "w-other".into(),
                title: "Other".into(),
                app: "App".into(),
                pid: 1,
                bounds: None,
                is_focused: true,
            },
            WindowInfo {
                id: "w-target".into(),
                title: "Target".into(),
                app: "App".into(),
                pid: 1,
                bounds: None,
                is_focused: false,
            },
        ])
    }

    fn get_tree(
        &self,
        win: &WindowInfo,
        _opts: &crate::adapter::TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        let children = if win.id == "w-target" {
            vec![AccessibilityNode {
                ref_id: None,
                role: "button".into(),
                name: Some("Saved!".into()),
                value: None,
                description: None,
                native_id: None,
                hint: None,
                states: vec![],
                available_actions: vec![],
                bounds: None,
                children_count: None,
                children: vec![],
            }]
        } else {
            vec![]
        };
        Ok(AccessibilityNode {
            ref_id: None,
            role: "window".into(),
            name: Some(win.title.clone()),
            value: None,
            description: None,
            native_id: None,
            hint: None,
            states: vec![],
            available_actions: vec![],
            bounds: None,
            children_count: None,
            children,
        })
    }
}

impl ActionOps for MultiWindowAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("ok"))
    }
}

impl InputOps for MultiWindowAdapter {}

impl SystemOps for MultiWindowAdapter {}

#[test]
fn post_action_wait_polls_acted_on_window_not_focused_window() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    let mut entry = entry();
    entry.source_app = Some("App".into());
    entry.source_window_id = Some("w-target".into());
    refmap.allocate(entry);
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":saved!".into(),
        gone: false,
        timeout_ms: 500,
    }));
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
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

#[test]
fn post_action_wait_without_flag_returns_action_only() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = ScopedWaitAdapter {
        request: Mutex::new(None),
        polled_app: Mutex::new(None),
    };
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
        timeout_ms: None,
    };

    let value = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["action"], "ok");
    assert!(value.get("after_action").is_none());
}

#[test]
fn post_action_wait_timeout_embeds_action_result_in_details() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    let mut entry = entry();
    entry.source_app = Some("TargetApp".into());
    refmap.allocate(entry);
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = ScopedWaitAdapter {
        request: Mutex::new(None),
        polled_app: Mutex::new(None),
    };
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":never-appears".into(),
        gone: false,
        timeout_ms: 50,
    }));
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
        timeout_ms: None,
    };

    let err = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &context,
    )
    .unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    let details = match err {
        crate::error::AppError::Adapter(adapter_err) => adapter_err.details.expect("details"),
        other => panic!("expected adapter timeout, got {other:?}"),
    };
    assert_eq!(details["kind"], "wait_timeout");
    assert_eq!(details["after_action"]["action"], "ok");
}
