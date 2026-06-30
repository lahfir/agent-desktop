use super::*;
use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    commands::query::parse_selector,
    context::CommandContext,
    error::{AdapterError, ErrorCode},
    node::{AccessibilityNode, WindowInfo},
    refs_store::RefStore,
    refs_test_support::HomeGuard,
};
use std::sync::atomic::{AtomicUsize, Ordering};

fn window_node(children: Vec<AccessibilityNode>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: "window".into(),
        name: Some("Doc".into()),
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children,
    }
}

fn button_node(label: &str) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: "button".into(),
        name: Some(label.into()),
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![],
    }
}

struct StaticTreeAdapter {
    tree: AccessibilityNode,
}

impl PlatformAdapter for StaticTreeAdapter {
    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "TestApp".into(),
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
        Ok(self.tree.clone())
    }
}

struct FlippingTreeAdapter {
    calls: AtomicUsize,
    before: AccessibilityNode,
    after: AccessibilityNode,
}

impl PlatformAdapter for FlippingTreeAdapter {
    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "TestApp".into(),
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
        let call = self.calls.fetch_add(1, Ordering::SeqCst);
        if call == 0 {
            Ok(self.before.clone())
        } else {
            Ok(self.after.clone())
        }
    }
}

struct ErrorThenTreeAdapter;

impl PlatformAdapter for ErrorThenTreeAdapter {
    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::new(ErrorCode::AppNotFound, "app missing"))
    }
}

fn base_input(query_raw: &str, gone: bool) -> WaitSelectorInput {
    WaitSelectorInput {
        query: parse_selector(query_raw),
        query_raw: query_raw.into(),
        gone,
        app: Some("TestApp".into()),
        window_id: None,
        opts: crate::adapter::TreeOptions::default(),
        timeout_ms: 500,
    }
}

#[test]
fn match_everything_selector_rejected() {
    let _guard = HomeGuard::new();
    let err = execute(
        WaitSelectorInput {
            query: parse_selector(""),
            query_raw: String::new(),
            gone: false,
            app: None,
            window_id: None,
            opts: crate::adapter::TreeOptions::default(),
            timeout_ms: 500,
        },
        &StaticTreeAdapter {
            tree: window_node(vec![]),
        },
        &CommandContext::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn present_on_first_poll_returns_snapshot_envelope() {
    let _guard = HomeGuard::new();
    let adapter = StaticTreeAdapter {
        tree: window_node(vec![button_node("saved")]),
    };
    let value = execute(
        base_input("button:saved", false),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    assert_eq!(value["matched_selector"], "button:saved");
    assert!(value["elapsed_ms"].as_u64().is_some());
    assert!(value["snapshot_id"].as_str().is_some());
    assert_eq!(value["ref_count"].as_u64(), Some(1));
}

#[test]
fn absent_then_present_on_second_poll() {
    let _guard = HomeGuard::new();
    let adapter = FlippingTreeAdapter {
        calls: AtomicUsize::new(0),
        before: window_node(vec![]),
        after: window_node(vec![button_node("saved")]),
    };
    let value = execute(
        base_input("button:saved", false),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    assert_eq!(value["matched_selector"], "button:saved");
    assert!(adapter.calls.load(Ordering::SeqCst) >= 2);
}

#[test]
fn gone_true_returns_when_element_disappears() {
    let _guard = HomeGuard::new();
    let adapter = FlippingTreeAdapter {
        calls: AtomicUsize::new(0),
        before: window_node(vec![button_node("spinner")]),
        after: window_node(vec![]),
    };
    let value = execute(
        base_input("button:spinner", true),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    assert_eq!(value["matched_selector"], "button:spinner");
}

#[test]
fn gone_true_when_never_present_returns_immediately() {
    let _guard = HomeGuard::new();
    let adapter = StaticTreeAdapter {
        tree: window_node(vec![]),
    };
    let value = execute(
        base_input(":missing", true),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    assert_eq!(value["matched_selector"], ":missing");
}

#[test]
fn timeout_includes_last_snapshot_id() {
    let _guard = HomeGuard::new();
    let adapter = StaticTreeAdapter {
        tree: window_node(vec![button_node("other")]),
    };
    let err = execute(
        WaitSelectorInput {
            timeout_ms: 50,
            ..base_input(":missing", false)
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "TIMEOUT");
    let details = match err {
        AppError::Adapter(adapter_err) => adapter_err.details.expect("timeout details"),
        other => panic!("expected adapter timeout, got {other:?}"),
    };
    assert_eq!(details["kind"], "wait_timeout");
    assert_eq!(details["predicate"], "selector");
    assert!(details["snapshot_id"].as_str().is_some());
    assert!(details.get("last_error").is_none() || details["last_error"].is_null());
}

#[test]
fn retryable_app_not_found_swallowed_until_timeout() {
    let _guard = HomeGuard::new();
    let err = execute(
        WaitSelectorInput {
            timeout_ms: 50,
            ..base_input("button:saved", false)
        },
        &ErrorThenTreeAdapter,
        &CommandContext::default(),
    )
    .unwrap_err();
    assert_eq!(err.code(), "TIMEOUT");
    let details = match err {
        AppError::Adapter(adapter_err) => adapter_err.details.expect("timeout details"),
        other => panic!("expected adapter timeout, got {other:?}"),
    };
    assert_eq!(details["last_error"]["code"], "APP_NOT_FOUND");
}

#[test]
fn persisted_snapshot_is_loadable() {
    let _guard = HomeGuard::new();
    let adapter = StaticTreeAdapter {
        tree: window_node(vec![button_node("saved")]),
    };
    let value = execute(
        base_input("button:saved", false),
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();
    let snapshot_id = value["snapshot_id"].as_str().unwrap();
    let store = RefStore::new().unwrap();
    let refmap = store.load(Some(snapshot_id)).unwrap();
    assert!(!refmap.is_empty());
}
