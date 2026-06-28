use crate::{
    action::{Action, KeyCombo, Modifier},
    action_request::ActionRequest,
    adapter::PlatformAdapter,
    error::{AdapterError, AppError},
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

fn canonical_combo(c: &KeyCombo) -> String {
    let mut mods: Vec<&str> = c
        .modifiers
        .iter()
        .map(|m| match m {
            Modifier::Cmd => "cmd",
            Modifier::Ctrl => "ctrl",
            Modifier::Alt => "alt",
            Modifier::Shift => "shift",
        })
        .collect();
    mods.sort_unstable();
    mods.dedup();
    mods.push(c.key.as_str());
    mods.join("+")
}

pub fn check_blocked_combo(raw: &str) -> Result<(), AppError> {
    let normalized = raw.to_lowercase().replace(' ', "");
    let Ok(parsed) = parse_combo(&normalized) else {
        return Ok(());
    };
    let canonical = canonical_combo(&parsed);
    for blocked in BLOCKED_COMBOS {
        if let Ok(blocked_parsed) = parse_combo(blocked) {
            if canonical == canonical_combo(&blocked_parsed) {
                return Err(AppError::Adapter(AdapterError::policy_denied(format!(
                    "Key combo '{raw}' is blocked for safety"
                ))));
            }
        }
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
#[path = "press_tests.rs"]
mod tests;
