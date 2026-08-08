//! `InputOps::key_event` and the keyboard-synthesis primitives it composes
//! with: modifier chords (`keyboard_event`) and chunked UTF-16 text
//! (`keyboard_text`). A standalone key edge has no daemon to own the hold
//! across a request boundary, so it rejects the same way macOS's
//! `reject_standalone_key_state` does (KTD7) - the atomic `press`/`type`
//! composers that call `synthesize_key`/`synthesize_text` under a focus
//! verify belong to the `execute_action` physical legs, not here.

use agent_desktop_core::{AdapterError, Deadline, ErrorCode, KeyCombo};

/// The chord primitive: press modifiers, press/release the key, release
/// modifiers, under a release guard. Not yet called from `execute_action` -
/// the physical `press`/`type` legs that compose it with a focus verify are
/// a separate seam - so it is reserved crate-internal surface, not dead
/// code.
#[allow(dead_code)]
pub(crate) fn synthesize_key(combo: &KeyCombo, deadline: Deadline) -> Result<(), AdapterError> {
    crate::input::keyboard_event::synthesize_key(combo, deadline)
}

/// The chunked UTF-16 `type_text` primitive. Reserved the same way
/// `synthesize_key` is: the physical `type` leg composes it with a focus
/// verify elsewhere.
#[allow(dead_code)]
pub(crate) fn synthesize_text(text: &str, deadline: Deadline) -> Result<(), AdapterError> {
    crate::input::keyboard_text::synthesize_text(text, deadline)
}

/// Deadline preflight for `synthesize_text`, callable independently so a
/// composing leg can reject before establishing focus.
#[allow(dead_code)]
pub(crate) fn preflight_text(text: &str, deadline: Deadline) -> Result<(), AdapterError> {
    crate::input::keyboard_text::preflight_text(text, deadline)
}

pub(crate) fn reject_standalone_key_state(
    _combo: &KeyCombo,
    _down: bool,
) -> Result<(), AdapterError> {
    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "Standalone key-down/key-up is unavailable in stateless mode",
    )
    .with_details(serde_json::json!({
        "raw_input_emitted": false,
        "requires_daemon_owned_transaction": true,
    }))
    .with_suggestion(
        "Use the atomic 'press' command; spanning key holds require a daemon-owned session that can release keys after disconnect",
    ))
}

#[cfg(test)]
#[path = "keyboard_tests.rs"]
mod tests;
