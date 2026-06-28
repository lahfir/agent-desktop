use super::{BLOCKED_COMBOS, check_blocked_combo, parse_combo};
use crate::action::Modifier;

#[test]
fn blocks_spaced_uppercase_variant() {
    assert!(check_blocked_combo("Cmd + Q").is_err());
}

#[test]
fn all_blocked_combos_entries_are_rejected_with_policy_denied() {
    for combo in BLOCKED_COMBOS {
        let err = check_blocked_combo(combo).unwrap_err();
        assert_eq!(
            err.code(),
            "POLICY_DENIED",
            "safety block of '{combo}' must surface as POLICY_DENIED, not INVALID_ARGS"
        );
    }
}

#[test]
fn reordered_blocked_combo_is_still_blocked() {
    assert!(
        check_blocked_combo("cmd+ctrl+q").is_err(),
        "cmd+ctrl+q must be blocked — modifier order must not affect the safety check"
    );
}

#[test]
fn aliased_blocked_combo_is_still_blocked() {
    assert!(
        check_blocked_combo("command+q").is_err(),
        "command+q must be blocked — 'command' is an alias for 'cmd'"
    );
}

#[test]
fn benign_combos_are_not_blocked() {
    for combo in ["cmd+c", "cmd+v", "cmd+shift+r", "cmd+w", "ctrl+s"] {
        assert!(
            check_blocked_combo(combo).is_ok(),
            "'{combo}' must not be blocked"
        );
    }
}

#[test]
fn parse_combo_single_modifier_and_key() {
    let combo = parse_combo("cmd+k").expect("cmd+k is valid");
    assert_eq!(combo.key, "k");
    assert_eq!(combo.modifiers, vec![Modifier::Cmd]);
}

#[test]
fn parse_combo_two_modifiers_preserved_in_declaration_order() {
    let combo = parse_combo("cmd+shift+t").expect("cmd+shift+t is valid");
    assert_eq!(combo.key, "t");
    assert_eq!(combo.modifiers, vec![Modifier::Cmd, Modifier::Shift]);
}

#[test]
fn parse_combo_bare_key_yields_empty_modifier_list() {
    let combo = parse_combo("return").expect("bare key is valid");
    assert_eq!(combo.key, "return");
    assert!(combo.modifiers.is_empty());
}

#[test]
fn parse_combo_accepts_long_form_modifier_aliases() {
    let cmd = parse_combo("command+a").expect("command alias");
    assert_eq!(cmd.modifiers, vec![Modifier::Cmd]);

    let alt = parse_combo("option+x").expect("option alias");
    assert_eq!(alt.modifiers, vec![Modifier::Alt]);

    let ctrl = parse_combo("control+y").expect("control alias");
    assert_eq!(ctrl.modifiers, vec![Modifier::Ctrl]);
}

#[test]
fn parse_combo_rejects_unknown_modifier_with_invalid_args_code() {
    let err = parse_combo("win+k").expect_err("unknown modifier must fail");
    assert_eq!(err.code(), "INVALID_ARGS");
    assert!(
        err.to_string().contains("win"),
        "error must name the unknown modifier, got: {}",
        err
    );
}

#[test]
fn parse_combo_rejects_empty_trailing_key() {
    let err = parse_combo("cmd+").expect_err("trailing + with no key must fail");
    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn parse_combo_key_is_preserved_verbatim_without_lowercasing() {
    let combo = parse_combo("cmd+K").expect("uppercase key is valid after lowercase modifier");
    assert_eq!(
        combo.key, "K",
        "parse_combo must NOT lowercase the key — normalization is the caller's responsibility"
    );
    assert_eq!(combo.modifiers, vec![Modifier::Cmd]);
}

#[test]
fn aliased_key_blocked_combo_is_still_blocked() {
    assert!(
        check_blocked_combo("cmd+shift+backspace").is_err(),
        "cmd+shift+backspace must be blocked — same OS action as cmd+shift+delete"
    );
    assert!(
        check_blocked_combo("cmd+alt+escape").is_err(),
        "cmd+alt+escape must be blocked — same OS action as cmd+alt+esc"
    );
}

#[test]
fn reordered_aliased_blocked_combo_is_still_blocked() {
    assert!(
        check_blocked_combo("shift+cmd+backspace").is_err(),
        "shift+cmd+backspace must be blocked — modifier order must not affect the safety check"
    );
}
