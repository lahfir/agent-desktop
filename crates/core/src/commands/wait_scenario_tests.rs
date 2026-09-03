use super::test_support::wait_args;
use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, ErrorCode, WindowInfo, adapter::WindowFilter};

struct TextlessTreeAdapter;

impl ObservationOps for TextlessTreeAdapter {
    fn observe_tree(
        &self,
        root: crate::live_locator::ObservationRoot<'_>,
        _request: &crate::live_locator::ObservationRequest,
    ) -> Result<crate::live_locator::ObservedTree, AdapterError> {
        crate::adapter::observed_tree(
            &root,
            crate::AccessibilityNode {
                ref_id: None,
                role: "window".into(),
                identity: crate::NodeIdentity {
                    name: Some("Doc".into()),
                    ..Default::default()
                },
                presentation: Default::default(),
                children_count: None,
                subtree_truncated: false,
                children: vec![],
            },
        )
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Ok(vec![WindowInfo {
            id: "w-1".into(),
            title: "Doc".into(),
            app: "TestApp".into(),
            pid: crate::ProcessId::new(1),
            process_instance: Some("test-instance".into()),
            bounds: None,
            state: crate::WindowState {
                is_focused: true,
                ..Default::default()
            },
        }])
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &crate::adapter::TreeOptions,
        _deadline: crate::Deadline,
    ) -> Result<crate::AccessibilityNode, AdapterError> {
        Ok(crate::AccessibilityNode {
            ref_id: None,
            role: "window".into(),
            identity: crate::NodeIdentity {
                name: Some("Doc".into()),
                ..Default::default()
            },
            presentation: Default::default(),
            children_count: None,
            subtree_truncated: false,
            children: vec![],
        })
    }
}

impl ActionOps for TextlessTreeAdapter {}

impl InputOps for TextlessTreeAdapter {}

impl SystemOps for TextlessTreeAdapter {}

#[test]
fn text_wait_with_count_zero_detects_absence() {
    let _guard = crate::refs_test_support::HomeGuard::new();

    let value = execute(
        WaitArgs {
            mode: WaitModeArgs {
                text: Some("Gone".into()),
                ..wait_args().mode
            },
            predicate: WaitPredicateArgs {
                count: Some(0),
                ..wait_args().predicate
            },
            timeout_ms: 1_000,
            app: Some("TestApp".into()),
        },
        &TextlessTreeAdapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(value["count"], 0);
}

fn ready_button(name: &str) -> crate::AccessibilityNode {
    crate::AccessibilityNode {
        ref_id: None,
        role: "button".into(),
        identity: crate::NodeIdentity {
            name: Some(name.into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: vec![],
    }
}

fn truncated_container(child_count: u32) -> crate::AccessibilityNode {
    crate::AccessibilityNode {
        ref_id: None,
        role: "group".into(),
        identity: crate::NodeIdentity {
            name: Some("list".into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: Some(child_count),
        subtree_truncated: true,
        children: vec![],
    }
}

fn doc_window(children: Vec<crate::AccessibilityNode>) -> crate::AccessibilityNode {
    crate::AccessibilityNode {
        ref_id: None,
        role: "window".into(),
        identity: crate::NodeIdentity {
            name: Some("Doc".into()),
            ..Default::default()
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

fn test_app_window() -> WindowInfo {
    WindowInfo {
        id: "w-1".into(),
        title: "Doc".into(),
        app: "TestApp".into(),
        pid: crate::ProcessId::new(1),
        process_instance: Some("test-instance".into()),
        bounds: None,
        state: crate::WindowState {
            is_focused: true,
            ..Default::default()
        },
    }
}

fn test_app_windows() -> Result<Vec<WindowInfo>, AdapterError> {
    Ok(vec![test_app_window()])
}

fn text_wait_args(text: &str, count: Option<usize>, timeout_ms: u64) -> WaitArgs {
    WaitArgs {
        mode: WaitModeArgs {
            text: Some(text.into()),
            ..wait_args().mode
        },
        predicate: WaitPredicateArgs {
            count,
            ..wait_args().predicate
        },
        timeout_ms,
        app: Some("TestApp".into()),
    }
}

fn expect_timeout(result: Result<Value, AppError>) -> crate::AdapterError {
    let AppError::Adapter(adapter_err) = result.unwrap_err() else {
        panic!("expected adapter error")
    };
    assert_eq!(adapter_err.code, ErrorCode::Timeout, "expected TIMEOUT");
    adapter_err
}

macro_rules! text_tree_adapter {
    ($name:ident, $tree:expr) => {
        struct $name;

        impl ObservationOps for $name {
            fn observe_tree(
                &self,
                root: crate::live_locator::ObservationRoot<'_>,
                _request: &crate::live_locator::ObservationRequest,
            ) -> Result<crate::live_locator::ObservedTree, AdapterError> {
                crate::adapter::observed_tree(&root, $tree)
            }

            fn list_windows(
                &self,
                _filter: &WindowFilter,
                _deadline: crate::Deadline,
            ) -> Result<Vec<WindowInfo>, AdapterError> {
                test_app_windows()
            }
        }

        impl ActionOps for $name {}

        impl InputOps for $name {}

        impl SystemOps for $name {}
    };
}

text_tree_adapter!(
    IncompleteTextTreeAdapter,
    doc_window(vec![ready_button("ready one"), truncated_container(2)])
);
text_tree_adapter!(
    AllHiddenIncompleteAdapter,
    doc_window(vec![truncated_container(2)])
);
text_tree_adapter!(
    CompleteTextTreeAdapter,
    doc_window(vec![
        ready_button("ready one"),
        ready_button("ready two"),
        ready_button("ready three"),
    ])
);

#[test]
fn text_wait_with_count_on_incomplete_observation_times_out() {
    let _guard = crate::refs_test_support::HomeGuard::new();

    let adapter_err = expect_timeout(execute(
        text_wait_args("ready", Some(1), 600),
        &IncompleteTextTreeAdapter,
        &CommandContext::default(),
    ));

    let details = adapter_err.details.expect("timeout carries details");
    assert_eq!(details["kind"], "wait_timeout");
    assert_eq!(details["expected_count"], 1);
}

#[test]
fn text_wait_with_count_zero_on_incomplete_observation_times_out() {
    let _guard = crate::refs_test_support::HomeGuard::new();

    let adapter_err = expect_timeout(execute(
        text_wait_args("ready", Some(0), 600),
        &AllHiddenIncompleteAdapter,
        &CommandContext::default(),
    ));

    let details = adapter_err.details.expect("timeout carries details");
    assert_eq!(details["kind"], "wait_timeout");
    assert_eq!(details["expected_count"], 0);
}

#[test]
fn text_wait_without_count_on_incomplete_observation_matches_visible() {
    let _guard = crate::refs_test_support::HomeGuard::new();

    let value = execute(
        text_wait_args("ready", None, 1_000),
        &IncompleteTextTreeAdapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(value["role"], "button");
    assert!(
        value.get("count").is_none(),
        "at-least-one waits must not report a count"
    );
}

#[test]
fn text_wait_without_count_on_all_hidden_incomplete_observation_times_out() {
    let _guard = crate::refs_test_support::HomeGuard::new();

    let adapter_err = expect_timeout(execute(
        text_wait_args("ready", None, 600),
        &AllHiddenIncompleteAdapter,
        &CommandContext::default(),
    ));

    let details = adapter_err.details.expect("timeout carries details");
    assert_eq!(details["kind"], "wait_timeout");
    assert!(details["expected_count"].is_null());
}

#[test]
fn text_wait_with_count_on_complete_observation_matches_exact_count() {
    let _guard = crate::refs_test_support::HomeGuard::new();

    let value = execute(
        text_wait_args("ready", Some(3), 1_000),
        &CompleteTextTreeAdapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(value["count"], 3);
}

#[test]
fn text_wait_with_count_on_complete_observation_times_out_when_count_differs() {
    let _guard = crate::refs_test_support::HomeGuard::new();

    let adapter_err = expect_timeout(execute(
        text_wait_args("ready", Some(1), 600),
        &CompleteTextTreeAdapter,
        &CommandContext::default(),
    ));

    let details = adapter_err.details.expect("timeout carries details");
    assert_eq!(details["kind"], "wait_timeout");
    assert_eq!(details["expected_count"], 1);
}

struct MenuWaitAdapter {
    open_seen: std::sync::Mutex<Option<bool>>,
}

impl ObservationOps for MenuWaitAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        Ok(vec![crate::AppInfo {
            name: "MenuApp".into(),
            pid: crate::ProcessId::new(42),
            bundle_id: None,
            process_instance: Some("test-instance".into()),
            presentation: None,
        }])
    }
}

impl ActionOps for MenuWaitAdapter {}

impl InputOps for MenuWaitAdapter {}

impl SystemOps for MenuWaitAdapter {
    fn wait_for_menu(
        &self,
        process: crate::ProcessIdentity,
        open: bool,
        _deadline: crate::Deadline,
    ) -> Result<(), AdapterError> {
        assert_eq!(process.pid, 42);
        assert_eq!(process.instance, "test-instance");
        *self.open_seen.lock().unwrap() = Some(open);
        Ok(())
    }
}

#[test]
fn menu_closed_wait_requests_closed_state_and_reports_found() {
    let adapter = MenuWaitAdapter {
        open_seen: std::sync::Mutex::new(None),
    };
    let value = execute(
        WaitArgs {
            mode: WaitModeArgs {
                surface: Some(SurfaceWait::MenuClosed),
                ..wait_args().mode
            },
            app: Some("MenuApp".into()),
            ..wait_args()
        },
        &adapter,
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["found"], true);
    assert_eq!(
        *adapter.open_seen.lock().unwrap(),
        Some(false),
        "--menu-closed must wait for the menu to be closed (open=false)"
    );
}
