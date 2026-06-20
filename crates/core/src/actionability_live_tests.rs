use super::*;
use crate::{
    action::Action,
    action_request::ActionRequest,
    adapter::{LiveElement, NativeHandle, PlatformAdapter, SnapshotSurface},
    capability,
    element_state::ElementState,
    node::Rect,
    refs::RefEntry,
};

struct LiveAdapter {
    state: Option<ElementState>,
    bounds: Option<Rect>,
    actions: Option<Vec<String>>,
}

impl PlatformAdapter for LiveAdapter {
    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Ok(self.state.clone())
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        Ok(self.bounds)
    }

    fn get_live_actions(
        &self,
        _handle: &NativeHandle,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        Ok(self.actions.clone())
    }
}

struct CombinedLiveAdapter;

impl PlatformAdapter for CombinedLiveAdapter {
    fn get_live_element(&self, _handle: &NativeHandle) -> Result<LiveElement, AdapterError> {
        Ok(LiveElement {
            state: Some(ElementState {
                role: "button".into(),
                states: vec![],
                value: None,
            }),
            bounds: Some(Rect {
                x: 1.0,
                y: 1.0,
                width: 20.0,
                height: 20.0,
            }),
            available_actions: Some(vec![capability::CLICK.into()]),
        })
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        panic!("check_live should use get_live_element")
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        panic!("check_live should use get_live_element")
    }

    fn get_live_actions(
        &self,
        _handle: &NativeHandle,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        panic!("check_live should use get_live_element")
    }
}

struct LiveReadErrorAdapter;

impl PlatformAdapter for LiveReadErrorAdapter {
    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Err(AdapterError::permission_denied())
    }
}

struct UnsupportedLiveAdapter;

impl PlatformAdapter for UnsupportedLiveAdapter {}

struct DeadLiveElementAdapter;

impl PlatformAdapter for DeadLiveElementAdapter {
    fn get_live_element(&self, _handle: &NativeHandle) -> Result<LiveElement, AdapterError> {
        Ok(LiveElement {
            state: Some(ElementState {
                role: "unknown".into(),
                states: vec![],
                value: None,
            }),
            bounds: None,
            available_actions: Some(vec![]),
        })
    }
}

fn entry() -> RefEntry {
    let bounds = Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    };
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("OK".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: Some(bounds),
        bounds_hash: Some(bounds.bounds_hash()),
        available_actions: vec![capability::CLICK.into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: true,
        path: smallvec::SmallVec::new(),
    }
}

#[test]
fn live_actionability_overrides_stale_snapshot_state() {
    let mut stale = entry();
    stale.states.push("disabled".into());
    let adapter = LiveAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec![],
            value: None,
        }),
        bounds: stale.bounds,
        actions: Some(vec![capability::CLICK.into()]),
    };

    let report = check_live(
        &stale,
        &NativeHandle::null(),
        &adapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap();

    assert!(report.actionable);
}

#[test]
fn live_actionability_uses_combined_live_element_read() {
    let mut stale = entry();
    stale.states.push("disabled".into());
    stale.bounds = Some(Rect {
        x: 1.0,
        y: 1.0,
        width: 0.0,
        height: 20.0,
    });
    stale.available_actions = vec![];

    let report = check_live(
        &stale,
        &NativeHandle::null(),
        &CombinedLiveAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap();

    assert!(report.actionable);
}

#[test]
fn live_actionability_uses_actions_gained_after_snapshot() {
    let mut stale = entry();
    stale.available_actions = vec![];
    let adapter = LiveAdapter {
        state: None,
        bounds: stale.bounds,
        actions: Some(vec![capability::CLICK.into()]),
    };

    let report = check_live(
        &stale,
        &NativeHandle::null(),
        &adapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap();

    assert!(report.actionable);
}

#[test]
fn live_actionability_fails_when_action_disappears_after_snapshot() {
    let stale = entry();
    let adapter = LiveAdapter {
        state: None,
        bounds: stale.bounds,
        actions: Some(vec![capability::SET_VALUE.into()]),
    };

    let err = check_live(
        &stale,
        &NativeHandle::null(),
        &adapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(err.code, ErrorCode::ActionFailed);
    assert!(err.message.contains("supported_action"));
}

#[test]
fn live_actionability_allows_identity_resolved_bounds_change() {
    let stale = entry();
    let adapter = LiveAdapter {
        state: None,
        bounds: Some(Rect {
            x: 100.0,
            y: 100.0,
            width: 20.0,
            height: 20.0,
        }),
        actions: Some(vec![capability::CLICK.into()]),
    };

    let report = check_live(
        &stale,
        &NativeHandle::null(),
        &adapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap();

    assert!(report.actionable);
    let stable = report
        .checks
        .iter()
        .find(|check| check.name == "stable")
        .unwrap();
    assert_eq!(stable.status, ActionabilityStatus::Unknown);
}

#[test]
fn empty_live_actions_do_not_erase_snapshot_capabilities() {
    let stale = entry();
    let adapter = LiveAdapter {
        state: None,
        bounds: stale.bounds,
        actions: Some(vec![]),
    };

    let report = check_live(
        &stale,
        &NativeHandle::null(),
        &adapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap();

    assert!(report.actionable);
}

#[test]
fn unsupported_live_reads_fall_back_to_snapshot_entry() {
    let report = check_live(
        &entry(),
        &NativeHandle::null(),
        &UnsupportedLiveAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap();

    assert!(report.actionable);
}

#[test]
fn empty_live_element_fails_as_stale_before_dispatch() {
    let err = check_live(
        &entry(),
        &NativeHandle::null(),
        &DeadLiveElementAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(err.code, ErrorCode::StaleRef);
    assert!(err.message.contains("no longer exposes live"));
}

#[test]
fn live_read_errors_are_not_silently_downgraded_to_snapshot_data() {
    let err = check_live(
        &entry(),
        &NativeHandle::null(),
        &LiveReadErrorAdapter,
        &ActionRequest::headless(Action::Click),
    )
    .unwrap_err();

    assert_eq!(err.code, ErrorCode::PermDenied);
}
