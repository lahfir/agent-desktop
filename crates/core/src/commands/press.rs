use crate::{
    action::{Action, KeyCombo, Modifier},
    action_request::ActionRequest,
    adapter::PlatformAdapter,
    error::AppError,
};
use serde_json::Value;

const BLOCKED_COMBOS: &[&str] = &[
    "cmd+q",
    "cmd+shift+q",
    "cmd+alt+esc",
    "ctrl+cmd+q",
    "cmd+shift+delete",
];

pub struct PressArgs {
    pub combo: String,
    pub app: Option<String>,
}

pub fn check_blocked_combo(raw: &str) -> Result<(), AppError> {
    let normalized = raw.to_lowercase().replace(' ', "");
    if BLOCKED_COMBOS.contains(&normalized.as_str()) {
        return Err(AppError::invalid_input(format!(
            "Key combo '{}' is blocked for safety",
            raw
        )));
    }
    Ok(())
}

pub fn execute(args: PressArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    check_blocked_combo(&args.combo)?;
    let normalized = args.combo.to_lowercase().replace(' ', "");
    let combo = parse_combo(&normalized)?;

    if let Some(app_name) = &args.app {
        let result = adapter.press_key_for_app(app_name, &combo)?;
        return Ok(serde_json::to_value(result)?);
    }

    let handle = crate::adapter::NativeHandle::null();
    let result = adapter.execute_action(&handle, ActionRequest::headed(Action::PressKey(combo)))?;
    Ok(serde_json::to_value(result)?)
}

pub fn parse_combo(s: &str) -> Result<KeyCombo, AppError> {
    let parts: Vec<&str> = s.split('+').collect();
    let key = parts
        .last()
        .copied()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| AppError::invalid_input("Empty key combo"))?
        .to_string();
    let mut modifiers = Vec::new();

    for &part in &parts[..parts.len() - 1] {
        let modifier = match part {
            "cmd" | "command" => Modifier::Cmd,
            "ctrl" | "control" => Modifier::Ctrl,
            "alt" | "option" => Modifier::Alt,
            "shift" => Modifier::Shift,
            other => {
                return Err(AppError::invalid_input(format!(
                    "Unknown modifier: '{other}'"
                )));
            }
        };
        modifiers.push(modifier);
    }

    Ok(KeyCombo { key, modifiers })
}

#[cfg(test)]
mod tests {
    use super::{BLOCKED_COMBOS, check_blocked_combo, parse_combo};
    use crate::action::Modifier;

    #[test]
    fn blocks_spaced_uppercase_variant() {
        assert!(check_blocked_combo("Cmd + Q").is_err());
    }

    #[test]
    fn all_blocked_combos_entries_are_rejected_with_invalid_args() {
        for combo in BLOCKED_COMBOS {
            let err = check_blocked_combo(combo).unwrap_err();
            assert_eq!(
                err.code(),
                "INVALID_ARGS",
                "safety block of '{combo}' must surface as INVALID_ARGS, not POLICY_DENIED"
            );
        }
    }

    #[test]
    fn order_sensitive_near_miss_is_allowed() {
        assert!(
            check_blocked_combo("cmd+ctrl+q").is_ok(),
            "cmd+ctrl+q must be allowed — the block list contains ctrl+cmd+q (different order)"
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
}
