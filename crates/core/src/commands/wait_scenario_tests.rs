use super::test_support::wait_args;
use super::*;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};
use crate::{AdapterError, WindowInfo, adapter::WindowFilter};

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
