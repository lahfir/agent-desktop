//! Windows-dangerous key combos.
//!
//! `is_blocked_combo`'s core default blocks nothing
//! (`crates/core/src/adapter/system.rs`); until this module is wired in,
//! Windows advertises a dangerous-shortcut guard in the skill docs that it
//! never enforced. The list is reasoned from Windows semantics, not
//! translated from macOS's `cmd+q`-shaped set, which has no Windows meaning:
//! `alt+f4` closes the active window, `win+l` locks the session, `win+d`
//! shows the desktop, and `alt+tab` steals the foreground mid-run. Every
//! modifier order and key-name alias of a blocked shortcut is caught by
//! canonicalizing before comparison, mirroring
//! `crates/macos/src/input/blocked_combo.rs`. `ctrl+alt+delete` is
//! deliberately absent: it is the Secure Attention Sequence, which
//! `SendInput` cannot synthesize at all, so listing it here would advertise
//! a guard this adapter does not provide. The calling agent can still send
//! any of these with `--force`, which core enforces.

use agent_desktop_core::{KeyCombo, Modifier};

const BLOCKED: &[&str] = &["alt+f4", "win+l", "win+d", "alt+tab"];

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
        Modifier::Meta => "win",
        Modifier::Ctrl => "ctrl",
        Modifier::Alt => "alt",
        Modifier::Shift => "shift",
    }
}

/// Canonicalizes a `mod+...+key` string for safety comparison: modifier
/// names are normalized and sorted (order-independent), and key names fold
/// to a single spelling per physical key, so every spelling and ordering of
/// a blocked shortcut matches. `meta`/`cmd` fold to `win` because the agent
/// may name the Windows key either way even though the shortcut itself only
/// exists on Windows.
fn canonical(raw: &str) -> String {
    let lower = raw.to_lowercase();
    let mut mods: Vec<&str> = Vec::new();
    let mut key = "";
    for part in lower.split('+') {
        match part {
            "win" | "meta" | "cmd" | "super" => mods.push("win"),
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
