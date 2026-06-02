use super::*;
use crate::{
    action::{Action, ActionRequest, Direction, ElementState},
    adapter::{NativeHandle, PlatformAdapter, SnapshotSurface},
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

fn entry() -> RefEntry {
    RefEntry {
        pid: 1,
        role: "button".into(),
        name: Some("OK".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: Some(Rect {
            x: 1.0,
            y: 1.0,
            width: 20.0,
            height: 20.0,
        }),
        bounds_hash: Some(1),
        available_actions: vec!["Click".into()],
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
fn click_passes_when_target_is_enabled_visible_and_supported() {
    let report = check(&entry(), &ActionRequest::headless(Action::Click)).unwrap();

    assert!(report.actionable);
}

#[test]
fn disabled_entry_fails_before_action_dispatch() {
    let mut entry = entry();
    entry.states.push("disabled".into());

    let err = check(&entry, &ActionRequest::headless(Action::Click)).unwrap_err();

    assert_eq!(err.code, ErrorCode::ActionFailed);
    assert!(err.message.contains("enabled"));
    assert_eq!(err.details.as_ref().unwrap()["actionable"], false);
}

#[test]
fn zero_sized_bounds_fail_visibility() {
    let mut entry = entry();
    entry.bounds = Some(Rect {
        x: 1.0,
        y: 1.0,
        width: 0.0,
        height: 20.0,
    });

    let err = check(&entry, &ActionRequest::headless(Action::Click)).unwrap_err();

    assert!(err.message.contains("visible"));
}

#[test]
fn text_input_requires_editable_target() {
    let err = check(
        &entry(),
        &ActionRequest::focus_fallback(Action::TypeText("hello".into())),
    )
    .unwrap_err();

    assert!(err.message.contains("editable"));
}

#[test]
fn cursor_movement_requires_physical_policy() {
    let err = check(&entry(), &ActionRequest::headless(Action::Hover)).unwrap_err();

    assert!(err.message.contains("policy"));
}

#[test]
fn command_aliases_match_platform_capabilities() {
    let click_entry = entry();
    assert!(check(&click_entry, &ActionRequest::headless(Action::DoubleClick)).is_ok());
    assert!(check(&click_entry, &ActionRequest::headless(Action::TripleClick)).is_ok());
    assert!(check(&click_entry, &ActionRequest::headless(Action::Check)).is_ok());
    assert!(check(&click_entry, &ActionRequest::headless(Action::Uncheck)).is_ok());

    let mut editable = entry();
    editable.role = "textfield".into();
    editable.available_actions = vec!["SetValue".into()];
    assert!(check(&editable, &ActionRequest::headless(Action::Clear)).is_ok());

    let mut scrollable = entry();
    scrollable.available_actions = vec!["Scroll".into()];
    assert!(
        check(
            &scrollable,
            &ActionRequest::headless(Action::Scroll(Direction::Down, 1))
        )
        .is_ok()
    );
    assert!(check(&scrollable, &ActionRequest::headless(Action::ScrollTo)).is_err());

    scrollable.available_actions = vec!["ScrollTo".into()];
    assert!(
        check(
            &scrollable,
            &ActionRequest::headless(Action::Scroll(Direction::Down, 1))
        )
        .is_ok()
    );
    assert!(check(&scrollable, &ActionRequest::headless(Action::ScrollTo)).is_ok());
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
        actions: Some(vec!["Click".into()]),
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
fn live_actionability_uses_actions_gained_after_snapshot() {
    let mut stale = entry();
    stale.available_actions = vec![];
    let adapter = LiveAdapter {
        state: None,
        bounds: stale.bounds,
        actions: Some(vec!["Click".into()]),
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
        actions: Some(vec![]),
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
