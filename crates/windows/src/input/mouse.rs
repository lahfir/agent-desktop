//! `InputOps::mouse_event`: `SendInput` mouse synthesis.
//!
//! Bare-coordinate path: `hover`/`mouse-move`/`mouse-click`/
//! `mouse-wheel` are all `--xy`, headed-only, and carry no target window to
//! verify, so there is no window-identity gate here - `--headed` is the
//! guard, enforced upstream. FFI reaches this method directly
//! (`crates/ffi/src/input/mouse.rs`), bypassing the core command layer, so
//! the guarantees core's commands validate are re-enforced here too: point
//! validity, click-count bounds, and the held-edge rejection.
//!
//! Delivery is best-effort throughout: `SendInput`'s return value is never
//! treated as evidence (A9-3), so every post here is fire-and-forget and
//! `Ok(())` follows injection rather than a re-read this method has no way
//! to perform for a bare coordinate.

use std::time::Duration;

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, MouseButton, MouseEvent, MouseEventKind,
    validate_mouse_click_count,
};

use crate::input::mouse_coord::{self, NormalizedPoint};
use crate::input::mouse_modifier::press_modifiers;
use crate::input::mouse_send::{MouseInputEvent, post_mouse_inputs};
use crate::system::permissions::ensure_budget;

pub(crate) const MOUSEEVENTF_MOVE: u32 = 0x0001;
pub(crate) const MOUSEEVENTF_LEFTDOWN: u32 = 0x0002;
pub(crate) const MOUSEEVENTF_LEFTUP: u32 = 0x0004;
const MOUSEEVENTF_RIGHTDOWN: u32 = 0x0008;
const MOUSEEVENTF_RIGHTUP: u32 = 0x0010;
const MOUSEEVENTF_MIDDLEDOWN: u32 = 0x0020;
const MOUSEEVENTF_MIDDLEUP: u32 = 0x0040;
const MOUSEEVENTF_WHEEL: u32 = 0x0800;
const MOUSEEVENTF_HWHEEL: u32 = 0x1000;
pub(crate) const MOUSEEVENTF_VIRTUALDESK: u32 = 0x4000;
pub(crate) const MOUSEEVENTF_ABSOLUTE: u32 = 0x8000;
const WHEEL_DELTA: i32 = 120;

#[cfg(target_os = "windows")]
const _: () = {
    use windows_sys::Win32::UI::Input::KeyboardAndMouse as win32;
    assert!(MOUSEEVENTF_MOVE == win32::MOUSEEVENTF_MOVE);
    assert!(MOUSEEVENTF_LEFTDOWN == win32::MOUSEEVENTF_LEFTDOWN);
    assert!(MOUSEEVENTF_LEFTUP == win32::MOUSEEVENTF_LEFTUP);
    assert!(MOUSEEVENTF_RIGHTDOWN == win32::MOUSEEVENTF_RIGHTDOWN);
    assert!(MOUSEEVENTF_RIGHTUP == win32::MOUSEEVENTF_RIGHTUP);
    assert!(MOUSEEVENTF_MIDDLEDOWN == win32::MOUSEEVENTF_MIDDLEDOWN);
    assert!(MOUSEEVENTF_MIDDLEUP == win32::MOUSEEVENTF_MIDDLEUP);
    assert!(MOUSEEVENTF_WHEEL == win32::MOUSEEVENTF_WHEEL);
    assert!(MOUSEEVENTF_HWHEEL == win32::MOUSEEVENTF_HWHEEL);
    assert!(MOUSEEVENTF_VIRTUALDESK == win32::MOUSEEVENTF_VIRTUALDESK);
    assert!(MOUSEEVENTF_ABSOLUTE == win32::MOUSEEVENTF_ABSOLUTE);
};

const MAX_LINES_PER_EVENT: i32 = 10;
const MAX_TOTAL_LINES: i32 = 1_000;
const CLICK_HOLD_DELAY: Duration = Duration::from_millis(10);
const CLICK_GAP_DELAY: Duration = Duration::from_millis(20);

pub(crate) fn synthesize_mouse(event: MouseEvent, deadline: Deadline) -> Result<(), AdapterError> {
    ensure_budget(deadline)?;
    event.point.validate()?;
    match event.kind {
        MouseEventKind::Down | MouseEventKind::Up => Err(standalone_state_error()),
        MouseEventKind::Move => dispatch_move(&event),
        MouseEventKind::Click { count } => {
            validate_mouse_click_count(count)?;
            dispatch_click(&event, count, deadline)
        }
        MouseEventKind::Wheel { delta_x, delta_y } => {
            dispatch_wheel(&event, delta_x, delta_y, deadline)
        }
    }
}

pub(crate) fn standalone_state_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "Standalone mouse-down/mouse-up is unavailable in stateless mode",
    )
    .with_details(serde_json::json!({
        "raw_input_emitted": false,
        "requires_daemon_owned_transaction": true,
    }))
    .with_suggestion(
        "Use atomic 'mouse-click' or 'drag'; spanning holds require a daemon-owned session that can release buttons after disconnect",
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
}

fn dispatch_move(event: &MouseEvent) -> Result<(), AdapterError> {
    let normalized = mouse_coord::normalize_point(&event.point);
    post_mouse_inputs(&[move_input(normalized)]);
    Ok(())
}

fn dispatch_click(event: &MouseEvent, count: u32, deadline: Deadline) -> Result<(), AdapterError> {
    ensure_budget(deadline)?;
    let normalized = mouse_coord::normalize_point(&event.point);
    let mut modifiers = press_modifiers(&event.modifiers);
    let result = post_click_sequence(&event.button, normalized, count, deadline);
    modifiers.release();
    result
}

fn post_click_sequence(
    button: &MouseButton,
    normalized: NormalizedPoint,
    count: u32,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    post_mouse_inputs(&[move_input(normalized)]);
    let (down_flag, up_flag) = button_flags(button);
    for click_index in 0..count {
        ensure_budget(deadline)?;
        post_mouse_inputs(&[button_input(down_flag)]);
        sleep_bounded(deadline, CLICK_HOLD_DELAY)?;
        post_mouse_inputs(&[button_input(up_flag)]);
        if click_index + 1 < count {
            sleep_bounded(deadline, CLICK_GAP_DELAY)?;
        }
    }
    Ok(())
}

fn dispatch_wheel(
    event: &MouseEvent,
    delta_x: f64,
    delta_y: f64,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    let y_chunks = signed_chunks(wheel_lines_to_i32(delta_y)?)?;
    let x_chunks = signed_chunks(wheel_lines_to_i32(delta_x)?)?;
    if y_chunks.is_empty() && x_chunks.is_empty() {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Wheel delta must be non-zero",
        ));
    }
    let normalized = mouse_coord::normalize_point(&event.point);
    post_mouse_inputs(&[move_input(normalized)]);
    for lines in y_chunks {
        ensure_budget(deadline)?;
        post_mouse_inputs(&[wheel_input(MOUSEEVENTF_WHEEL, lines)]);
    }
    for lines in x_chunks {
        ensure_budget(deadline)?;
        post_mouse_inputs(&[wheel_input(MOUSEEVENTF_HWHEEL, lines)]);
    }
    Ok(())
}

fn move_input(normalized: NormalizedPoint) -> MouseInputEvent {
    let mut flags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE;
    if normalized.virtual_desktop {
        flags |= MOUSEEVENTF_VIRTUALDESK;
    }
    MouseInputEvent {
        dx: normalized.x,
        dy: normalized.y,
        mouse_data: 0,
        flags,
    }
}

fn button_input(flag: u32) -> MouseInputEvent {
    MouseInputEvent {
        dx: 0,
        dy: 0,
        mouse_data: 0,
        flags: flag,
    }
}

fn wheel_input(flag: u32, lines: i32) -> MouseInputEvent {
    MouseInputEvent {
        dx: 0,
        dy: 0,
        mouse_data: lines.saturating_mul(WHEEL_DELTA),
        flags: flag,
    }
}

fn button_flags(button: &MouseButton) -> (u32, u32) {
    match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    }
}

fn wheel_lines_to_i32(delta: f64) -> Result<i32, AdapterError> {
    if !delta.is_finite() || delta < f64::from(i32::MIN) || delta > f64::from(i32::MAX) {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Wheel line delta must be a finite 32-bit value",
        ));
    }
    let rounded = delta.round();
    if rounded == 0.0 && delta != 0.0 {
        return Ok(if delta.is_sign_positive() { 1 } else { -1 });
    }
    Ok(rounded as i32)
}

fn signed_chunks(value: i32) -> Result<Vec<i32>, AdapterError> {
    if value == 0 {
        return Ok(Vec::new());
    }
    if value.unsigned_abs() > MAX_TOTAL_LINES as u32 {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Wheel delta must be within -1000..=1000 lines per axis",
        ));
    }
    let mut remaining = value;
    let mut chunks = Vec::new();
    while remaining != 0 {
        let next = remaining.clamp(-MAX_LINES_PER_EVENT, MAX_LINES_PER_EVENT);
        chunks.push(next);
        remaining -= next;
    }
    Ok(chunks)
}

pub(crate) fn sleep_bounded(deadline: Deadline, duration: Duration) -> Result<(), AdapterError> {
    let capped = deadline.remaining_slice(duration)?;
    std::thread::sleep(capped);
    ensure_budget(deadline)
}

#[cfg(all(test, target_os = "windows"))]
#[path = "mouse_tests.rs"]
mod tests;
