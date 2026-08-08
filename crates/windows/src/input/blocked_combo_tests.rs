use super::*;

fn combo(key: &str, modifiers: Vec<Modifier>) -> KeyCombo {
    KeyCombo {
        key: key.into(),
        modifiers,
    }
}

#[test]
fn every_windows_dangerous_combo_is_blocked() {
    assert!(is_blocked(&combo("f4", vec![Modifier::Alt])), "alt+f4");
    assert!(is_blocked(&combo("l", vec![Modifier::Meta])), "win+l");
    assert!(is_blocked(&combo("d", vec![Modifier::Meta])), "win+d");
    assert!(is_blocked(&combo("tab", vec![Modifier::Alt])), "alt+tab");
}

#[test]
fn a_non_dangerous_combo_is_never_blocked() {
    assert!(!is_blocked(&combo("c", vec![Modifier::Ctrl])));
    assert!(!is_blocked(&combo("a", vec![])));
    assert!(!is_blocked(&combo(
        "s",
        vec![Modifier::Ctrl, Modifier::Shift]
    )));
}

/// The Secure Attention Sequence is not presented as guarded: `SendInput`
/// cannot synthesize it, so claiming to block it would be dishonest.
#[test]
fn ctrl_alt_delete_is_not_claimed_as_blocked() {
    assert!(!is_blocked(&combo(
        "delete",
        vec![Modifier::Ctrl, Modifier::Alt]
    )));
}

#[test]
fn modifier_reordering_does_not_evade_the_block() {
    let reordered = combo("f4", vec![Modifier::Alt]);
    assert!(is_blocked(&reordered));
    assert_eq!(
        canonical(&combo_to_string(&reordered)),
        canonical("f4+alt"),
        "modifier position must not change the canonical form"
    );
}

#[test]
fn key_name_aliases_do_not_evade_the_block() {
    assert_eq!(canonical("alt+f4"), canonical("ALT+F4"));
    assert_eq!(canonical("win+l"), canonical("meta+l"));
    assert_eq!(canonical("win+d"), canonical("cmd+d"));
}

#[test]
fn canonical_key_folds_documented_aliases() {
    assert_eq!(canonical_key("escape"), "esc");
    assert_eq!(canonical_key("esc"), "esc");
    assert_eq!(canonical_key("backspace"), "delete");
    assert_eq!(canonical_key("delete"), "delete");
    assert_eq!(canonical_key("enter"), "return");
    assert_eq!(canonical_key("return"), "return");
}

#[test]
fn combo_to_string_places_modifiers_before_the_key() {
    let text = combo_to_string(&combo("f4", vec![Modifier::Alt]));
    assert_eq!(text, "alt+f4");
}

#[test]
fn extra_unrelated_modifiers_are_not_folded_into_a_blocked_shortcut() {
    assert!(!is_blocked(&combo(
        "f4",
        vec![Modifier::Alt, Modifier::Shift]
    )));
}
