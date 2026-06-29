use crate::{
    action::{KeyCombo, Modifier},
    adapter::PlatformAdapter,
    error::{AdapterError, AppError},
};

/// Refuses `combo` when the platform adapter reports it as dangerous, unless
/// the caller forced it. Whether a combo is dangerous is the adapter's
/// decision (`is_blocked_combo`); core only enforces the verdict and honors
/// the `--force` override, so the calling agent always retains control.
pub fn ensure_combo_allowed(
    combo: &KeyCombo,
    raw: &str,
    force: bool,
    adapter: &dyn PlatformAdapter,
) -> Result<(), AppError> {
    if !force && adapter.is_blocked_combo(combo) {
        return Err(AppError::Adapter(AdapterError::policy_denied(format!(
            "Key combo '{raw}' is blocked for safety; pass --force to override"
        ))));
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
