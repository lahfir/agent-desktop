use super::is_blocked;
use agent_desktop_core::action::{KeyCombo, Modifier};

fn combo(modifiers: Vec<Modifier>, key: &str) -> KeyCombo {
    KeyCombo {
        key: key.to_owned(),
        modifiers,
    }
}

#[test]
fn dangerous_shortcuts_are_blocked() {
    assert!(is_blocked(&combo(vec![Modifier::Cmd], "q")));
    assert!(is_blocked(&combo(
        vec![Modifier::Cmd, Modifier::Shift],
        "q"
    )));
    assert!(is_blocked(&combo(
        vec![Modifier::Cmd, Modifier::Alt],
        "esc"
    )));
    assert!(is_blocked(&combo(vec![Modifier::Ctrl, Modifier::Cmd], "q")));
    assert!(is_blocked(&combo(
        vec![Modifier::Cmd, Modifier::Shift],
        "delete"
    )));
}

#[test]
fn modifier_order_does_not_matter() {
    assert!(
        is_blocked(&combo(vec![Modifier::Cmd, Modifier::Ctrl], "q")),
        "cmd+ctrl+q must match the blocked ctrl+cmd+q regardless of order"
    );
}

#[test]
fn key_aliases_are_blocked() {
    assert!(
        is_blocked(&combo(vec![Modifier::Cmd, Modifier::Alt], "escape")),
        "escape is the same physical key as esc"
    );
    assert!(
        is_blocked(&combo(vec![Modifier::Cmd, Modifier::Shift], "backspace")),
        "backspace is the same physical key as delete"
    );
}

#[test]
fn benign_combos_are_not_blocked() {
    assert!(!is_blocked(&combo(vec![Modifier::Cmd], "c")));
    assert!(!is_blocked(&combo(vec![Modifier::Cmd], "v")));
    assert!(!is_blocked(&combo(vec![Modifier::Cmd], "w")));
    assert!(!is_blocked(&combo(
        vec![Modifier::Cmd, Modifier::Shift],
        "r"
    )));
    assert!(!is_blocked(&combo(vec![Modifier::Ctrl], "s")));
    assert!(!is_blocked(&combo(vec![], "return")));
}
