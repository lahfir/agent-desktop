use crate::{
    action::{KeyCombo, Modifier},
    error::{AdapterError, AppError},
};

pub(crate) const BLOCKED_COMBOS: &[&str] = &[
    "cmd+q",
    "cmd+shift+q",
    "cmd+alt+esc",
    "cmd+alt+escape",
    "ctrl+cmd+q",
    "cmd+shift+delete",
    "cmd+shift+backspace",
];

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

pub fn parse_combo_normalized(raw: &str) -> Result<KeyCombo, AppError> {
    let normalized = raw.to_lowercase().replace(' ', "");
    parse_combo(&normalized)
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
#[path = "combo_tests.rs"]
mod tests;
