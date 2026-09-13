use super::*;
use crate::AdapterError;
use crate::adapter::{ActionOps, InputOps, ObservationOps, SystemOps};

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
    use super::test_support::wait_args;
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
