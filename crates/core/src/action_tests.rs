use super::*;
use crate::interaction_policy::InteractionPolicy;

fn dummy_key() -> KeyCombo {
    KeyCombo {
        key: "a".into(),
        modifiers: vec![],
    }
}

fn dummy_drag() -> DragParams {
    DragParams {
        from: Point { x: 0.0, y: 0.0 },
        to: Point { x: 1.0, y: 1.0 },
        duration_ms: None,
        drop_delay_ms: None,
    }
}

#[test]
fn action_names_do_not_include_payloads() {
    let cases = [
        (Action::SetValue("private".into()), "set-value"),
        (Action::Select("private".into()), "select"),
        (Action::TypeText("private".into()), "type"),
        (
            Action::PressKey(KeyCombo {
                key: "A".into(),
                modifiers: vec![Modifier::Cmd],
            }),
            "press",
        ),
    ];

    for (action, expected) in cases {
        assert_eq!(action.name(), expected);
    }
}

#[test]
fn pure_ax_actions_base_policy_is_headless() {
    let headless = InteractionPolicy::headless();
    let pure_ax: &[Action] = &[
        Action::Click,
        Action::DoubleClick,
        Action::RightClick,
        Action::TripleClick,
        Action::SetFocus,
        Action::Expand,
        Action::Collapse,
        Action::Toggle,
        Action::Check,
        Action::Uncheck,
        Action::ScrollTo,
        Action::Clear,
        Action::Scroll(Direction::Down, 3),
        Action::SetValue("v".into()),
        Action::Select("s".into()),
    ];
    for action in pure_ax {
        assert_eq!(
            action.base_interaction_policy(),
            headless,
            "{} must use headless base policy",
            action.name()
        );
    }
}

#[test]
fn press_key_and_type_text_base_policy_is_focus_fallback() {
    let focus = InteractionPolicy::focus_fallback();
    assert_eq!(
        Action::PressKey(KeyCombo {
            key: "a".into(),
            modifiers: vec![Modifier::Cmd],
        })
        .base_interaction_policy(),
        focus,
        "PressKey must request focus_fallback to land in the right field"
    );
    assert_eq!(
        Action::TypeText("hello".into()).base_interaction_policy(),
        focus,
        "TypeText must request focus_fallback"
    );
}

#[test]
fn key_down_and_key_up_base_policy_is_headless_unlike_press_key() {
    let headless = InteractionPolicy::headless();
    assert_eq!(
        Action::KeyDown(dummy_key()).base_interaction_policy(),
        headless,
        "KeyDown must be headless; raw key-down events do not need focus theft"
    );
    assert_eq!(
        Action::KeyUp(dummy_key()).base_interaction_policy(),
        headless,
        "KeyUp must be headless"
    );
}

#[test]
fn hover_and_drag_base_policy_is_headless_independent_of_cursor_requirement() {
    let headless = InteractionPolicy::headless();
    assert_eq!(
        Action::Hover.base_interaction_policy(),
        headless,
        "Hover base_interaction_policy is headless even though requires_cursor_policy is true"
    );
    assert_eq!(
        Action::Drag(dummy_drag()).base_interaction_policy(),
        headless,
        "Drag base_interaction_policy is headless even though requires_cursor_policy is true"
    );
    assert!(
        Action::Hover.requires_cursor_policy(),
        "Hover.requires_cursor_policy() must still be true"
    );
    assert!(
        Action::Drag(dummy_drag()).requires_cursor_policy(),
        "Drag.requires_cursor_policy() must still be true"
    );
}

#[test]
fn requires_hit_test_covers_ref_targeted_pointer_actions() {
    let hit_tested: &[Action] = &[
        Action::Click,
        Action::DoubleClick,
        Action::RightClick,
        Action::TripleClick,
        Action::Hover,
        Action::Drag(dummy_drag()),
    ];
    for action in hit_tested {
        assert!(
            action.requires_hit_test(),
            "{} must require a hit test before dispatch",
            action.name()
        );
    }

    let not_hit_tested: &[Action] = &[
        Action::SetValue("v".into()),
        Action::SetFocus,
        Action::Expand,
        Action::Collapse,
        Action::Select("s".into()),
        Action::Toggle,
        Action::Check,
        Action::Uncheck,
        Action::Scroll(Direction::Down, 1),
        Action::ScrollTo,
        Action::PressKey(dummy_key()),
        Action::KeyDown(dummy_key()),
        Action::KeyUp(dummy_key()),
        Action::TypeText("t".into()),
        Action::Clear,
    ];
    for action in not_hit_tested {
        assert!(
            !action.requires_hit_test(),
            "{} must not require a hit test",
            action.name()
        );
    }
}

/// F10 regression coverage: `MouseEvent.modifiers` must stay `#[serde(default)]`
/// so a legacy payload recorded before modifiers existed (or any FFI/batch
/// caller that omits the key) still deserializes instead of erroring out.
#[test]
fn mouse_event_json_without_modifiers_key_deserializes_to_empty() {
    let event: MouseEvent = serde_json::from_value(serde_json::json!({
        "kind": { "Click": { "count": 1 } },
        "point": { "x": 1.0, "y": 2.0 },
        "button": "Left",
    }))
    .unwrap();

    assert!(event.modifiers.is_empty());
}

#[test]
fn mouse_event_json_with_modifiers_round_trips() {
    let event = MouseEvent {
        kind: MouseEventKind::Click { count: 2 },
        point: Point { x: 1.0, y: 2.0 },
        button: MouseButton::Left,
        modifiers: vec![Modifier::Cmd, Modifier::Shift],
    };

    let json = serde_json::to_value(&event).unwrap();
    let round_tripped: MouseEvent = serde_json::from_value(json).unwrap();

    assert_eq!(
        round_tripped.modifiers,
        vec![Modifier::Cmd, Modifier::Shift]
    );
}
