use crate::{
    action::{Action, KeyCombo, Modifier},
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
}

pub fn execute(args: PressArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let normalized = args.combo.to_lowercase().replace(' ', "");
    if BLOCKED_COMBOS.contains(&normalized.as_str()) {
        return Err(AppError::invalid_input(format!(
            "Key combo '{}' is blocked for safety",
            args.combo
        )));
    }

    let combo = parse_combo(&normalized)?;
    let handle = crate::adapter::NativeHandle::null();
    let result = adapter.execute_action(&handle, Action::PressKey(combo))?;
    Ok(serde_json::to_value(result)?)
}

fn parse_combo(s: &str) -> Result<KeyCombo, AppError> {
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
                )))
            }
        };
        modifiers.push(modifier);
    }

    Ok(KeyCombo { key, modifiers })
}
