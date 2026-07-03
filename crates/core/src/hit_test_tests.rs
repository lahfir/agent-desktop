use super::*;
use crate::{
    action::Action,
    action_request::ActionRequest,
    actionability::{ActionabilityCheck, ActionabilityStatus, check_live},
    adapter::{ActionOps, InputOps, LiveElement, NativeHandle, ObservationOps, SystemOps},
    element_state::ElementState,
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
};
use smallvec::SmallVec;

struct HitTestAdapter {
    outcome: Result<HitTestResult, AdapterError>,
}

impl ObservationOps for HitTestAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_element(&self, _handle: &NativeHandle) -> Result<LiveElement, AdapterError> {
        Ok(LiveElement {
            state: Some(ElementState {
                role: "button".into(),
                states: vec![],
                value: None,
            }),
            bounds: Some(crate::node::Rect {
                x: 10.0,
                y: 10.0,
                width: 40.0,
                height: 20.0,
            }),
            available_actions: Some(vec!["Click".into()]),
        })
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::action::Point,
    ) -> Result<HitTestResult, AdapterError> {
        self.outcome.clone()
    }
}

impl ActionOps for HitTestAdapter {}
impl InputOps for HitTestAdapter {}
impl SystemOps for HitTestAdapter {}

fn clickable_entry() -> RefEntry {
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("Save".into()),
        value: None,
        description: None,
        native_id: None,
        states: vec![],
        bounds: Some(crate::node::Rect {
            x: 10.0,
            y: 10.0,
            width: 40.0,
            height: 20.0,
        }),
        bounds_hash: Some(1),
        available_actions: vec!["Click".into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: SmallVec::new(),
    }
}

/// Runs `check_live` for a `Click` (which requires a hit test) against the
/// given hit-test outcome and returns the `receives_events` check. Only the
/// `InterceptedBy` outcome fails actionability, so every other outcome is
/// safe to unwrap here; the `InterceptedBy` case is asserted separately
/// against the `Err` path so its occluder details can be inspected.
fn run_receives_events_check(outcome: Result<HitTestResult, AdapterError>) -> ActionabilityCheck {
    let adapter = HitTestAdapter { outcome };
    let entry = clickable_entry();
    let request = ActionRequest::headless(Action::Click);
    let report = check_live(&entry, &NativeHandle::null(), &adapter, &request)
        .expect("only an InterceptedBy hit-test outcome fails actionability");
    report
        .checks
        .into_iter()
        .find(|check| check.name == "receives_events")
        .expect("Click requires a receives_events check")
}

#[test]
fn reaches_target_result_passes_receives_events_check() {
    let check = run_receives_events_check(Ok(HitTestResult::ReachesTarget));
    assert_eq!(check.status, ActionabilityStatus::Pass);
}

#[test]
fn unknown_hit_test_result_does_not_block_action() {
    let check = run_receives_events_check(Ok(HitTestResult::Unknown));
    assert_eq!(check.status, ActionabilityStatus::Unknown);
}

#[test]
fn not_supported_hit_test_does_not_block_action() {
    let check = run_receives_events_check(Err(AdapterError::not_supported("hit_test")));
    assert_eq!(check.status, ActionabilityStatus::Unknown);
}

#[test]
fn hit_test_probe_error_does_not_block_action() {
    let check = run_receives_events_check(Err(AdapterError::internal(
        "AXUIElementCopyElementAtPosition failed",
    )));
    assert_eq!(
        check.status,
        ActionabilityStatus::Unknown,
        "a probe failure must never be reported as a Fail"
    );
}

#[test]
fn intercepted_by_result_fails_and_carries_redactable_occluder() {
    let adapter = HitTestAdapter {
        outcome: Ok(HitTestResult::InterceptedBy {
            role: Some("AXSheet".into()),
            name: Some("Save changes?".into()),
            bounds: None,
        }),
    };
    let entry = clickable_entry();
    let request = ActionRequest::headless(Action::Click);

    let err = check_live(&entry, &NativeHandle::null(), &adapter, &request)
        .expect_err("a hit outside the target's ancestor chain must fail actionability");

    assert_eq!(err.code, ErrorCode::ActionFailed);
    assert!(err.message.contains("AXSheet"));
    assert!(!err.message.contains("Save changes?"));

    let details = err
        .details
        .expect("a Fail report attaches actionability details");
    let checks = details["checks"]
        .as_array()
        .expect("details.checks is an array");
    let receives_events = checks
        .iter()
        .find(|check| check["name"] == "receives_events")
        .expect("receives_events check must be present when Click requires a hit test");
    assert_eq!(receives_events["status"], "fail");
    assert_eq!(receives_events["occluder"]["name"], "Save changes?");
    assert_eq!(receives_events["occluder"]["role"], "AXSheet");
}
