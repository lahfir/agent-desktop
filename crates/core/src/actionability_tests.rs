use super::*;
use crate::{
    action::{Action, Direction},
    action_request::ActionRequest,
    adapter::SnapshotSurface,
    capability,
    node::Rect,
    refs::RefEntry,
};

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
    let bounds = Rect {
        x: 1.0,
        y: 1.0,
        width: 0.0,
        height: 20.0,
    };
    entry.bounds = Some(bounds);
    entry.bounds_hash = Some(bounds.bounds_hash());

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
fn headless_type_text_fails_policy_before_dispatch() {
    let mut target = entry();
    target.role = "textfield".into();
    target.available_actions = vec![capability::SET_VALUE.into()];

    let err = check(
        &target,
        &ActionRequest::headless(Action::TypeText("x".into())),
    )
    .unwrap_err();

    assert!(err.message.contains("policy"));
    assert!(err.message.contains("focus"));
}

#[test]
fn right_click_requires_right_click_capability_before_dispatch() {
    let err = check(&entry(), &ActionRequest::headless(Action::RightClick)).unwrap_err();

    assert_eq!(err.code, ErrorCode::ActionFailed);
    assert!(err.message.contains("supported_action"));
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
    editable.available_actions = vec![capability::SET_VALUE.into()];
    assert!(check(&editable, &ActionRequest::headless(Action::Clear)).is_ok());

    let mut scrollable = entry();
    scrollable.available_actions = vec![capability::SCROLL.into()];
    assert!(
        check(
            &scrollable,
            &ActionRequest::headless(Action::Scroll(Direction::Down, 1))
        )
        .is_ok()
    );
    assert!(check(&scrollable, &ActionRequest::headless(Action::ScrollTo)).is_err());

    scrollable.available_actions = vec![capability::SCROLL_TO.into()];
    assert!(
        check(
            &scrollable,
            &ActionRequest::headless(Action::Scroll(Direction::Down, 1))
        )
        .is_ok()
    );
    assert!(check(&scrollable, &ActionRequest::headless(Action::ScrollTo)).is_ok());
}
