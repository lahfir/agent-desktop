use agent_desktop_core::action::{KeyCombo, Modifier};

const BLOCKED: &[&str] = &[
    "cmd+q",
    "cmd+shift+q",
    "cmd+alt+esc",
    "ctrl+cmd+q",
    "cmd+shift+delete",
];

/// Reports whether `combo` is one of the macOS shortcuts that would quit, log
/// out, force-quit, or lock the session. Comparison is canonical, so every
/// modifier order and key-name alias of a blocked shortcut is caught. The
/// calling agent can still send any of these by passing `--force`.
pub(crate) fn is_blocked(combo: &KeyCombo) -> bool {
    let target = canonical(&combo_to_string(combo));
    BLOCKED.iter().any(|entry| canonical(entry) == target)
}

fn combo_to_string(combo: &KeyCombo) -> String {
    let mut parts: Vec<&str> = combo.modifiers.iter().map(modifier_name).collect();
    parts.push(combo.key.as_str());
    parts.join("+")
}

fn modifier_name(modifier: &Modifier) -> &'static str {
    match modifier {
        Modifier::Cmd => "cmd",
        Modifier::Ctrl => "ctrl",
        Modifier::Alt => "alt",
        Modifier::Shift => "shift",
    }
}

/// Canonicalizes a `mod+...+key` string for safety comparison: modifier names
/// are normalized and sorted (order-independent), and key names are folded to a
/// single spelling per physical key (macOS aliases esc/escape, delete/backspace,
/// enter/return), so every spelling and ordering of a blocked shortcut matches.
fn canonical(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let mut mods: Vec<&str> = Vec::new();
    let mut key = "";
    for part in lower.split('+') {
        match part {
            "cmd" | "command" => mods.push("cmd"),
            "ctrl" | "control" => mods.push("ctrl"),
            "alt" | "option" => mods.push("alt"),
            "shift" => mods.push("shift"),
            other => key = canonical_key(other),
        }
    }
    mods.sort_unstable();
    mods.dedup();
    mods.push(key);
    mods.join("+")
}

fn canonical_key(key: &str) -> &str {
    match key {
        "escape" | "esc" => "esc",
        "backspace" | "delete" => "delete",
        "enter" | "return" => "return",
        other => other,
    }
}

#[cfg(test)]
#[path = "blocked_combo_tests.rs"]
mod tests;
