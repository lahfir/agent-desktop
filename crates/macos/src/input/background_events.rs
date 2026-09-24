use agent_desktop_core::{AdapterError, ErrorCode, MouseButton, MouseEvent, MouseEventKind};
use core_graphics::event::{CGEvent, CGEventFlags, CGEventType, EventField};
use core_graphics::geometry::CGPoint;
use std::time::Duration;

use crate::input::mouse::{
    create_event_with_source, down_type, event_source, standalone_state_error, to_cg_button,
    up_type,
};

/// Undocumented field Warp sets to the target window number next to 91/92.
pub(crate) const FIELD_WINDOW_NUMBER: u32 = 51;
/// Undocumented field Warp sets to `1` and cua uses as a click-group id.
pub(crate) const FIELD_CLICK_GROUP: u32 = 58;
/// `NSEventSubtypeTouch`; cua and background-computer-use both tag
/// pid-targeted clicks with it so AppKit does not treat them as tablet input.
pub(crate) const SUBTYPE_TOUCH: i64 = 3;

const BUTTON_HOLD: Duration = Duration::from_millis(10);
const MULTI_CLICK_GAP: Duration = Duration::from_millis(30);
const PRE_CLICK_SETTLE: Duration = Duration::from_millis(15);
const PRIMER_SETTLE: Duration = Duration::from_millis(100);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Move,
    Down,
    Up,
}

/// One event of a background delivery, planned before any is built or posted
/// so ordering, click states, and pauses are unit-testable.
#[derive(Clone, Debug)]
pub(crate) struct PlannedEvent {
    pub(crate) phase: Phase,
    pub(crate) button: MouseButton,
    pub(crate) global: CGPoint,
    pub(crate) click_state: i64,
    pub(crate) pause_after: Duration,
}

/// Plans a hover/move as one `mouseMoved`. A click becomes an optional
/// preceding move at the target (`route`, so Chromium sees the pointer enter
/// first), an optional primer click at window-local `(-1, -1)` when
/// `primer_origin` is set, then one down/up pair per click with click states
/// `1..=count`.
///
/// The primer sits one point above and left of the window frame, outside any
/// content view, so it cannot press a real control; it exists only to let
/// Chromium consume its first-mouse handling before the real click.
pub(crate) fn plan(
    event: &MouseEvent,
    route: bool,
    primer_origin: Option<CGPoint>,
) -> Result<Vec<PlannedEvent>, AdapterError> {
    let target = CGPoint::new(event.point.x, event.point.y);
    let planned = |phase, button: &MouseButton, global, click_state, pause_after| PlannedEvent {
        phase,
        button: button.clone(),
        global,
        click_state,
        pause_after,
    };

    let count = match event.kind {
        MouseEventKind::Move => {
            return Ok(vec![planned(
                Phase::Move,
                &event.button,
                target,
                0,
                Duration::ZERO,
            )]);
        }
        MouseEventKind::Click { count } => count,
        MouseEventKind::Down | MouseEventKind::Up => return Err(standalone_state_error()),
        MouseEventKind::Wheel { .. } => {
            return Err(AdapterError::new(
                ErrorCode::ActionNotSupported,
                "Background pointer delivery supports move and click only",
            ));
        }
    };
    agent_desktop_core::validate_mouse_click_count(count)?;

    let mut events = Vec::new();
    if route {
        events.push(planned(
            Phase::Move,
            &event.button,
            target,
            0,
            PRE_CLICK_SETTLE,
        ));
    }
    if let Some(origin) = primer_origin {
        let primer = CGPoint::new(origin.x - 1.0, origin.y - 1.0);
        events.push(planned(
            Phase::Down,
            &MouseButton::Left,
            primer,
            1,
            BUTTON_HOLD,
        ));
        events.push(planned(
            Phase::Up,
            &MouseButton::Left,
            primer,
            1,
            PRIMER_SETTLE,
        ));
    }
    for click_state in 1..=i64::from(count) {
        let gap = if click_state == i64::from(count) {
            Duration::ZERO
        } else {
            MULTI_CLICK_GAP
        };
        events.push(planned(
            Phase::Down,
            &event.button,
            target,
            click_state,
            BUTTON_HOLD,
        ));
        events.push(planned(Phase::Up, &event.button, target, click_state, gap));
    }
    Ok(events)
}

/// Routing inputs shared by every event of one delivery.
#[derive(Clone, Copy, Debug)]
pub(crate) struct EventRouting {
    pub(crate) pid: libc::pid_t,
    pub(crate) window_number: i64,
    pub(crate) flags: CGEventFlags,
    pub(crate) route: bool,
}

/// Builds, without posting, the `CGEvent` for `planned`. Fields 91/92 and
/// the click state are always set. The `route` layer adds the target pid
/// (40), Warp's window/click-group fields (51/58), the button number, the
/// touch subtype, and pressure (1.0 while a button is down, as Warp does).
/// The window-local location needs a private symbol and is applied by the
/// caller.
pub(crate) fn build(
    planned: &PlannedEvent,
    routing: &EventRouting,
) -> Result<CGEvent, AdapterError> {
    let event_type = match planned.phase {
        Phase::Move => CGEventType::MouseMoved,
        Phase::Down => down_type(&planned.button),
        Phase::Up => up_type(&planned.button),
    };
    let source = event_source()?;
    let button = to_cg_button(&planned.button);
    let event =
        create_event_with_source(&source, event_type, planned.global, button, routing.flags)?;

    event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, planned.click_state);
    event.set_integer_value_field(
        EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER,
        routing.window_number,
    );
    event.set_integer_value_field(
        EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT,
        routing.window_number,
    );
    if !routing.route {
        return Ok(event);
    }

    let pressure = if planned.phase == Phase::Down {
        1.0
    } else {
        0.0
    };
    event.set_integer_value_field(
        EventField::EVENT_TARGET_UNIX_PROCESS_ID,
        i64::from(routing.pid),
    );
    event.set_integer_value_field(FIELD_WINDOW_NUMBER, routing.window_number);
    event.set_integer_value_field(FIELD_CLICK_GROUP, 1);
    event.set_integer_value_field(
        EventField::MOUSE_EVENT_BUTTON_NUMBER,
        button_number(&planned.button),
    );
    event.set_integer_value_field(EventField::MOUSE_EVENT_SUB_TYPE, SUBTYPE_TOUCH);
    event.set_double_value_field(EventField::MOUSE_EVENT_PRESSURE, pressure);
    Ok(event)
}

/// Converts a global top-left CoreGraphics point into the window-local
/// top-left point `CGEventSetWindowLocation` expects. Origins can be negative
/// on displays left of or above the main display.
pub(crate) fn window_local(global: CGPoint, window_origin: CGPoint) -> CGPoint {
    CGPoint::new(global.x - window_origin.x, global.y - window_origin.y)
}

fn button_number(button: &MouseButton) -> i64 {
    match button {
        MouseButton::Left => 0,
        MouseButton::Right => 1,
        MouseButton::Middle => 2,
    }
}

#[cfg(test)]
#[path = "background_events_tests.rs"]
mod tests;
