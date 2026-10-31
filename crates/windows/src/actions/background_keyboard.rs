//! Strict-headless `type` for Chromium and Electron windows.
//!
//! `SendInput` needs the target to own the desktop foreground, so the
//! physical leg needs `--headed`. A Chromium top-level window
//! (`Chrome_WidgetWin_1`) also accepts keyboard input as window messages
//! once it has been told it is active, which leaves the foreground window
//! and the physical keyboard alone. The leg activates the window by message,
//! focuses the target, sends the text, and deactivates it again, unless the
//! window became the foreground window in the meantime. Any other window
//! is left to the physical leg, which refuses it under a headless policy.

use std::time::{Duration, Instant};

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode, StepMechanism,
};

use crate::actions::background_target as target;
use crate::actions::physical_target::{ensure_elevation_allows, read_click_bounds};
use crate::input::keyboard::keyboard_message::{
    activation_messages, click_messages, text_messages,
};
use crate::input::keyboard::keyboard_message_send::send_window_messages;
use crate::input::keyboard::preflight_text;
use crate::tree::element::UIAElement;
use crate::tree::live_read::corroborate_verified_process;

const TYPE_TEXT_LABEL: &str = "WindowMessage.type_text";
const FOCUS_BUDGET: Duration = Duration::from_millis(500);
const CARET_BUDGET: Duration = Duration::from_millis(500);
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Types `text` by window messages when `element` lives in a Chromium
/// window, and returns `None` for any other window.
pub(crate) fn background_type_steps(
    element: &UIAElement,
    text: &str,
    deadline: Deadline,
) -> Result<Option<Vec<ActionStep>>, AdapterError> {
    preflight_text(text, deadline)?;
    corroborate_verified_process(element)?;
    let Some(root) = target::chromium_root(element, deadline)? else {
        return Ok(None);
    };
    ensure_elevation_allows(element)?;
    if target::is_minimized(root) {
        return Err(minimized_window());
    }
    let plan = BackgroundTypePlan {
        root,
        was_foreground: target::is_foreground(root),
        text,
    };
    type_text_by_messages(
        plan,
        deadline,
        |deadline| focus_target(element, root, deadline),
        || !target::is_foreground(root),
    )
    .map(|step| Some(vec![step]))
}

/// What `type_text_by_messages` delivers, bundled so it stays within the
/// parameter cap.
pub(crate) struct BackgroundTypePlan<'a> {
    pub(crate) root: isize,
    pub(crate) was_foreground: bool,
    pub(crate) text: &'a str,
}

/// The message sequence past the window checks, with target focusing and
/// the foreground re-read injected so a test drives the same code the
/// adapter runs. Deactivation is sent whenever activation was, even when a
/// later step fails, so the window is never left believing it is active.
pub(crate) fn type_text_by_messages(
    plan: BackgroundTypePlan<'_>,
    deadline: Deadline,
    focus_target: impl FnOnce(Deadline) -> Result<(), AdapterError>,
    still_background: impl FnOnce() -> bool,
) -> Result<ActionStep, AdapterError> {
    let activate = !plan.was_foreground;
    if activate {
        let activation = activation_messages(true);
        let delivered = send_window_messages(plan.root, &activation, deadline);
        if delivered < activation.len() {
            deactivate(plan.root, deadline);
            return Err(stopped_before_text(deadline));
        }
    }
    let result = focus_target(deadline).and_then(|()| send_text(plan.root, plan.text, deadline));
    if activate && still_background() {
        deactivate(plan.root, deadline);
    }
    result.map(|()| {
        ActionStep::succeeded(TYPE_TEXT_LABEL)
            .with_mechanism(StepMechanism::PhysicalSynthetic)
            .with_verified(false)
    })
}

fn send_text(root: isize, text: &str, deadline: Deadline) -> Result<(), AdapterError> {
    let messages = text_messages(text);
    let delivered = send_window_messages(root, &messages, deadline);
    if delivered < messages.len() {
        return Err(partial_delivery(delivered, messages.len(), deadline));
    }
    Ok(())
}

fn deactivate(root: isize, deadline: Deadline) {
    send_window_messages(root, &activation_messages(false), deadline);
}

/// Focuses the target with a click message on its center. Chromium handles
/// the click before the key messages that follow it, because both travel the
/// same input queue, so no character can reach the element that held focus
/// before. A field that already holds text then gets the caret at its end
/// through its UIA text range, once UIA reports keyboard focus on it.
/// Chromium applies that selection on another channel than input messages,
/// so the leg waits until no text follows the caret before it types. A page
/// that a covering window has hidden can report focus late; when focus or
/// the caret cannot be confirmed in time, the text goes in at the click
/// point.
fn focus_target(element: &UIAElement, root: isize, deadline: Deadline) -> Result<(), AdapterError> {
    let bounds = read_click_bounds(element)?;
    let center_x = (bounds.x + bounds.width / 2.0).round() as i32;
    let center_y = (bounds.y + bounds.height / 2.0).round() as i32;
    let (x, y) =
        target::screen_to_client(root, center_x, center_y).ok_or_else(window_unresponsive)?;
    let click = click_messages(x, y);
    if send_window_messages(root, &click, deadline) < click.len() {
        return Err(stopped_before_text(deadline));
    }
    let focused = wait_until(deadline, FOCUS_BUDGET, || {
        target::has_keyboard_focus(element)
    });
    if focused && target::holds_text(element) && target::select_end_of_text(element) {
        wait_until(deadline, CARET_BUDGET, || target::caret_at_end(element));
    }
    Ok(())
}

fn wait_until(deadline: Deadline, budget: Duration, mut ready: impl FnMut() -> bool) -> bool {
    let started = Instant::now();
    loop {
        if ready() {
            return true;
        }
        if deadline.is_expired() || started.elapsed() >= budget {
            return false;
        }
        std::thread::sleep(POLL_INTERVAL.min(deadline.remaining()));
    }
}

fn minimized_window() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Chromium does not expose or accept input for a minimized window",
    )
    .with_suggestion("Restore the window, then retry. It can stay behind other windows.")
    .with_disposition(DeliverySemantics::not_delivered())
}

fn window_unresponsive() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "The target window did not accept input messages",
    )
    .with_suggestion("Run 'snapshot' to check the window, then retry.")
    .with_disposition(DeliverySemantics::not_delivered())
}

fn stopped_before_text(deadline: Deadline) -> AdapterError {
    if deadline.is_expired() {
        return deadline
            .timeout_error()
            .with_disposition(DeliverySemantics::not_delivered());
    }
    window_unresponsive()
}

fn partial_delivery(delivered: usize, planned: usize, deadline: Deadline) -> AdapterError {
    let disposition = if delivered == 0 {
        DeliverySemantics::not_delivered()
    } else {
        DeliverySemantics::delivered_unverified()
    };
    stopped_before_text(deadline)
        .with_details(serde_json::json!({
            "delivered_messages": delivered,
            "planned_messages": planned,
        }))
        .with_disposition(disposition)
}

#[cfg(all(test, target_os = "windows"))]
#[path = "background_keyboard_tests.rs"]
mod tests;
