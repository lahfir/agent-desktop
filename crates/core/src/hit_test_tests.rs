use super::*;
use crate::{
    action::Action,
    action_request::ActionRequest,
    actionability::check_live,
    adapter::{ActionOps, InputOps, LiveElement, NativeHandle, ObservationOps, SystemOps},
    element_state::ElementState,
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
};
use smallvec::SmallVec;

struct HitTestAdapter {
    receives: bool,
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
        Ok(if self.receives {
            HitTestResult::receives_events(Some("AXButton".into()))
        } else {
            HitTestResult::blocked(Some("AXGroup".into()))
        })
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

#[test]
fn occluded_target_fails_receives_events_check() {
    let adapter = HitTestAdapter { receives: false };
    let entry = clickable_entry();
    let request = ActionRequest::headless(Action::Click);
    let err = check_live(&entry, &NativeHandle::null(), &adapter, &request)
        .expect_err("occluded targets must fail actionability");
    assert_eq!(err.code, ErrorCode::ActionFailed);
    assert!(err.message.contains("receives_events"));
}

#[test]
fn unoccluded_target_passes_receives_events_check() {
    let adapter = HitTestAdapter { receives: true };
    let entry = clickable_entry();
    let request = ActionRequest::headless(Action::Click);
    check_live(&entry, &NativeHandle::null(), &adapter, &request).expect("clickable target");
}
