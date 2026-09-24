use agent_desktop_core::{AdapterError, BackgroundKeyInput, KeyCombo};
use core_graphics::event::{CGEvent, CGEventFlags, EventField};
use std::time::Duration;

use crate::input::background_events::FIELD_WINDOW_NUMBER;
use crate::input::keyboard_map::key_name_to_code;
use crate::input::mouse::{event_flags, event_source};

/// Hold between key down and key up of a combo.
const COMBO_HOLD: Duration = Duration::from_millis(8);
/// Hold between key down and key up of one typed character (cua uses 8 ms).
const TEXT_HOLD: Duration = Duration::from_millis(8);
/// Pause after each typed character so the target consumes the pair before
/// the next one arrives; bursts without it lost characters in Electron.
const TEXT_GAP: Duration = Duration::from_millis(8);

const RETURN_KEY: u16 = 36;
const TAB_KEY: u16 = 48;
/// Key code for every typed character other than Return and Tab. The
/// Unicode string, not the key code, carries the character, as in cua,
/// background-computer-use, and Warp, so no keyboard layout is assumed.
const TEXT_KEY: u16 = 0;

/// One key event of a background delivery, planned before any event is
/// built or posted so ordering, key codes, text, and pacing are testable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlannedKey {
    pub(crate) key_code: u16,
    pub(crate) down: bool,
    pub(crate) flags: CGEventFlags,
    /// UTF-16 text the event carries; empty lets the target derive the
    /// characters from the key code and its own layout.
    pub(crate) text: Vec<u16>,
    pub(crate) pause_after: Duration,
}

/// Plans a combo as one key down/up pair with the modifiers as flags on both
/// events, and text as one pair per character carrying that character as a
/// Unicode string. Line breaks become Return (`\r\n` counts once) and tabs
/// become Tab, so editors see the real keys.
pub(crate) fn plan(input: &BackgroundKeyInput) -> Result<Vec<PlannedKey>, AdapterError> {
    match input {
        BackgroundKeyInput::Combo(combo) => plan_combo(combo),
        BackgroundKeyInput::Text(text) => Ok(plan_text(text)),
    }
}

fn plan_combo(combo: &KeyCombo) -> Result<Vec<PlannedKey>, AdapterError> {
    let key_code = key_name_to_code(&combo.key)?;
    let flags = event_flags(&combo.modifiers);
    Ok(key_pair(
        key_code,
        flags,
        combo_text(key_code),
        COMBO_HOLD,
        Duration::ZERO,
    )
    .to_vec())
}

/// Return and Tab carry the same Unicode text as a typed line break or tab.
/// Without it the combo reaches AppKit with empty `characters`, which a
/// background VS Code ignored for Return while it honored a typed `\n`.
/// Other keys keep an empty string so the target derives the characters
/// from the key code and its own layout.
fn combo_text(key_code: u16) -> Vec<u16> {
    match key_code {
        RETURN_KEY => vec![u16::from(b'\r')],
        TAB_KEY => vec![u16::from(b'\t')],
        _ => Vec::new(),
    }
}

fn plan_text(text: &str) -> Vec<PlannedKey> {
    text.replace("\r\n", "\n")
        .chars()
        .flat_map(|character| {
            let (key_code, flags, units) = text_key(character);
            key_pair(key_code, flags, units, TEXT_HOLD, TEXT_GAP)
        })
        .collect()
}

fn key_pair(
    key_code: u16,
    flags: CGEventFlags,
    text: Vec<u16>,
    hold: Duration,
    gap: Duration,
) -> [PlannedKey; 2] {
    let down = PlannedKey {
        key_code,
        down: true,
        flags,
        text,
        pause_after: hold,
    };
    let up = PlannedKey {
        down: false,
        pause_after: gap,
        ..down.clone()
    };
    [down, up]
}

/// The key and UTF-16 text for one typed character: Return and Tab use
/// their real keys so editors run their line-break and indent handling;
/// every other character rides on [`TEXT_KEY`] with no modifier flags.
fn text_key(character: char) -> (u16, CGEventFlags, Vec<u16>) {
    match character {
        '\n' | '\r' => (RETURN_KEY, CGEventFlags::empty(), vec![u16::from(b'\r')]),
        '\t' => (TAB_KEY, CGEventFlags::empty(), vec![u16::from(b'\t')]),
        _ => {
            let mut buffer = [0_u16; 2];
            let units = character.encode_utf16(&mut buffer).to_vec();
            (TEXT_KEY, CGEventFlags::empty(), units)
        }
    }
}

/// Routing inputs shared by every event of one delivery.
#[derive(Clone, Copy, Debug)]
pub(crate) struct KeyRouting {
    pub(crate) pid: libc::pid_t,
    pub(crate) window_number: i64,
    pub(crate) route: bool,
}

/// Builds, without posting, the keyboard `CGEvent` for `planned`. The `route`
/// layer adds the target pid (field 40) and the window number in field 51,
/// the same fields the pointer path uses, so AppKit can associate the event
/// with the exact window instead of whatever it considers key.
pub(crate) fn build(planned: &PlannedKey, routing: &KeyRouting) -> Result<CGEvent, AdapterError> {
    let source = event_source()?;
    let event = CGEvent::new_keyboard_event(source, planned.key_code, planned.down)
        .map_err(|()| AdapterError::internal("CGEvent::new_keyboard_event failed"))?;
    event.set_flags(planned.flags);
    if !planned.text.is_empty() {
        event.set_string_from_utf16_unchecked(&planned.text);
    }
    if routing.route {
        event.set_integer_value_field(
            EventField::EVENT_TARGET_UNIX_PROCESS_ID,
            i64::from(routing.pid),
        );
        event.set_integer_value_field(FIELD_WINDOW_NUMBER, routing.window_number);
    }
    Ok(event)
}

#[cfg(test)]
#[path = "background_key_events_tests.rs"]
mod tests;
