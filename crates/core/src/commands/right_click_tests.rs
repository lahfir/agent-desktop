use super::*;
use crate::{
    action::ActionResult,
    adapter::{NativeHandle, WindowFilter},
    error::{AdapterError, ErrorCode},
    node::WindowInfo,
    refs::{RefEntry, RefMap},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};

struct ProbeFailingAdapter {
    tree_error: Option<ErrorCode>,
}

impl PlatformAdapter for ProbeFailingAdapter {
    fn resolve_element(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("right_click"))
    }

    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        if filter.app.is_some() && self.tree_error.is_none() {
            return Err(AdapterError::new(
                ErrorCode::WindowNotFound,
                "menu probe failed",
            ));
        }
        if filter.focused_only {
            return Err(AdapterError::new(
                ErrorCode::WindowNotFound,
                "no focused menu",
            ));
        }
        Ok(vec![WindowInfo {
            id: "w1".into(),
            title: "Main".into(),
            app: "TargetApp".into(),
            pid: 7,
            bounds: None,
            is_focused: true,
        }])
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &TreeOptions,
    ) -> Result<crate::node::AccessibilityNode, AdapterError> {
        if let Some(code) = self.tree_error.clone() {
            return Err(AdapterError::new(code, "menu tree unavailable"));
        }
        Ok(crate::node::AccessibilityNode {
            ref_id: None,
            role: "menu".into(),
            name: None,
            value: None,
            description: None,
            hint: None,
            states: Vec::new(),
            available_actions: Vec::new(),
            bounds: None,
            children_count: None,
            children: Vec::new(),
        })
    }
}

fn save_refmap(source_app: Option<String>) -> String {
    let mut refmap = RefMap::new();
    refmap.allocate(RefEntry {
        pid: 7,
        role: "button".into(),
        name: Some("Open".into()),
        value: None,
        description: None,
        states: Vec::new(),
        bounds: None,
        bounds_hash: None,
        available_actions: vec!["RightClick".into()],
        source_app,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    });
    RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap()
}

#[test]
fn returns_action_success_when_menu_probe_fails() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_refmap(None);

    let value = execute(
        RefArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
        },
        &ProbeFailingAdapter { tree_error: None },
    )
    .unwrap();

    assert_eq!(value["action"], "right_click");
    assert_eq!(value["menu_probe"]["ok"], false);
    assert_eq!(value["menu_probe"]["error"]["code"], "WINDOW_NOT_FOUND");
}

#[test]
fn element_not_found_menu_probe_uses_right_click_specific_guidance() {
    let _guard = HomeGuard::new();
    let snapshot_id = save_refmap(Some("TargetApp".into()));

    let value = execute(
        RefArgs {
            ref_id: "@e1".into(),
            snapshot_id: Some(snapshot_id),
        },
        &ProbeFailingAdapter {
            tree_error: Some(ErrorCode::ElementNotFound),
        },
    )
    .unwrap();

    assert_eq!(value["action"], "right_click");
    assert_eq!(value["menu_probe"]["ok"], false);
    assert_eq!(value["menu_probe"]["error"]["code"], "ELEMENT_NOT_FOUND");
    assert!(
        value["menu_probe"]["error"]["suggestion"]
            .as_str()
            .unwrap()
            .contains("snapshot --surface menu")
    );
}
