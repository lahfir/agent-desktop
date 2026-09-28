use super::{PressArgs, execute};
use crate::action_request::ActionRequest;
use crate::action_result::ActionResult;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::context::CommandContext;
use crate::{AdapterError, KeyCombo};
use std::sync::Mutex;

#[derive(Default)]
struct TwoInstancesAdapter {
    pressed_pid: Mutex<Option<u32>>,
}

fn instance(pid: u32) -> crate::AppInfo {
    crate::AppInfo {
        name: "Electron".into(),
        pid: crate::ProcessId::new(pid),
        bundle_id: Some("com.github.Electron".into()),
        process_instance: Some(format!("generation-{pid}")),
        presentation: None,
    }
}

fn window(pid: u32) -> crate::WindowInfo {
    crate::WindowInfo {
        id: format!("w-{pid}"),
        title: format!("Instance {pid}"),
        app: "Electron".into(),
        pid: crate::ProcessId::new(pid),
        process_instance: Some(format!("generation-{pid}")),
        bounds: None,
        state: crate::WindowState {
            visible: Some(true),
            ..Default::default()
        },
    }
}

impl ObservationOps for TwoInstancesAdapter {
    fn list_apps(&self, _deadline: crate::Deadline) -> Result<Vec<crate::AppInfo>, AdapterError> {
        Ok(vec![instance(41), instance(42)])
    }

    fn list_windows(
        &self,
        _filter: &crate::WindowFilter,
        _deadline: crate::Deadline,
    ) -> Result<Vec<crate::WindowInfo>, AdapterError> {
        Ok(vec![window(41), window(42)])
    }
}

impl ActionOps for TwoInstancesAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        panic!("a targeted press must never fall back to global delivery")
    }
}

impl InputOps for TwoInstancesAdapter {}

impl SystemOps for TwoInstancesAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn press_key_for_app(
        &self,
        process: crate::ProcessIdentity,
        _combo: &KeyCombo,
        _policy: crate::InteractionPolicy,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        *self.pressed_pid.lock().unwrap() = Some(process.pid.get());
        Ok(ActionResult::delivered_unverified("PressKey"))
    }
}

fn press(
    app: Option<&str>,
    window_id: Option<&str>,
    adapter: &TwoInstancesAdapter,
) -> Result<serde_json::Value, crate::AppError> {
    execute(
        PressArgs {
            combo: "cmd+k".into(),
            app: app.map(str::to_owned),
            window_id: window_id.map(str::to_owned),
            force: false,
        },
        adapter,
        &CommandContext::default(),
    )
}

#[test]
fn window_id_reaches_the_instance_that_owns_the_window() {
    let adapter = TwoInstancesAdapter::default();

    press(None, Some("w-42"), &adapter).unwrap();

    assert_eq!(*adapter.pressed_pid.lock().unwrap(), Some(42));
}

#[test]
fn window_id_narrows_an_app_name_shared_by_two_instances() {
    let adapter = TwoInstancesAdapter::default();

    press(Some("Electron"), Some("w-41"), &adapter).unwrap();

    assert_eq!(*adapter.pressed_pid.lock().unwrap(), Some(41));
}

#[test]
fn app_name_alone_stays_ambiguous_across_instances() {
    let adapter = TwoInstancesAdapter::default();

    let error = press(Some("Electron"), None, &adapter).unwrap_err();

    assert_eq!(error.code(), "AMBIGUOUS_TARGET");
    assert!(adapter.pressed_pid.lock().unwrap().is_none());
}

#[test]
fn unknown_window_id_is_window_not_found() {
    let adapter = TwoInstancesAdapter::default();

    let error = press(None, Some("w-7"), &adapter).unwrap_err();

    assert_eq!(error.code(), "WINDOW_NOT_FOUND");
    assert!(adapter.pressed_pid.lock().unwrap().is_none());
}
