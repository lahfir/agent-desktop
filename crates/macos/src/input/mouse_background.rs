use agent_desktop_core::{
    AdapterError, BackgroundDeliveryReport, Deadline, MouseEvent, WindowInfo,
};
use core_graphics::geometry::CGPoint;

use crate::actions::DeliveryTracker;
use crate::input::background_delivery::{
    Prepared, SystemDeliveryIo, note_once, run, window_number,
};
use crate::input::background_events::{EventRouting, Phase, build, plan, window_local};
use crate::input::background_layers::BackgroundLayers;
use crate::input::mouse::{event_flags, validate_point};
use crate::input::prepared_event::PreparedEvent;
use crate::input::skylight;

/// Delivers a mouse event to the process that owns `window` without the HID
/// tap, so the system cursor stays put and the window server hit-tests
/// nothing: offscreen or covered windows are fine.
///
/// Fields 91/92 name the exact window and the click state is always set;
/// [`BackgroundLayers`] (from `AGENT_DESKTOP_BG_LAYERS`) adds the rest. Why a
/// bare `CGEventPostToPid` was not enough (live on macOS 26: no effect in
/// Finder, VS Code, or ClickUp): AppKit routes a pid-targeted event by its
/// window fields and window-local location, and Chromium ignores `mouseMoved`
/// and first-mouse clicks unless its window is main/key in its own app
/// (`render_widget_host_view_cocoa.mm`). `route` supplies the missing routing,
/// `activate` makes the target believe its window is focused without touching
/// the user's app, `skylight` posts the way the window server's own clients
/// do, and `guard` puts the user's app back if the target activates itself.
/// None of this is guaranteed: the target may still ignore the events or
/// activate itself, so callers must observe the effect with a fresh snapshot.
pub(crate) fn deliver(
    window: &WindowInfo,
    event: MouseEvent,
    deadline: Deadline,
) -> Result<BackgroundDeliveryReport, AdapterError> {
    let prepared =
        prepare(window, &event).map_err(|error| DeliveryTracker::default().annotate(error))?;
    let mut io = SystemDeliveryIo::new(deadline);
    let mut report = run(prepared, deadline, &mut io)?;
    io.note_degradations(&mut report.degradations);
    Ok(report)
}

/// Everything that can fail runs here, before anything is posted, so those
/// failures are reported as not delivered.
fn prepare(window: &WindowInfo, event: &MouseEvent) -> Result<Prepared, AdapterError> {
    validate_point(&event.point)?;
    let layers = BackgroundLayers::pointer_from_env()?;
    let window_number = window_number(window)?;
    let pid = crate::system::process_identity::to_pid_t(window.pid)?;

    let origin = window
        .bounds
        .as_ref()
        .map(|bounds| CGPoint::new(bounds.x, bounds.y));
    let mut degradations = Vec::new();
    if origin.is_none() && (layers.route || layers.primer) {
        degradations.push("route:window_origin_unavailable".to_string());
    }

    let planned = plan(event, layers.route, origin.filter(|_| layers.primer))?;
    let routing = EventRouting {
        pid,
        window_number: i64::from(window_number),
        flags: event_flags(&event.modifiers),
        route: layers.route,
    };
    let mut events = Vec::with_capacity(planned.len());
    for planned_event in &planned {
        let built = build(planned_event, &routing)?;
        if let Some(origin) = origin.filter(|_| layers.route) {
            let local = window_local(planned_event.global, origin);
            if !skylight::set_window_location(&built, local) {
                note_once(
                    &mut degradations,
                    "route:CGEventSetWindowLocation_unavailable",
                );
            }
        }
        events.push(PreparedEvent {
            event: built,
            pause_after: planned_event.pause_after,
            completes_press: planned_event.phase == Phase::Up,
        });
    }

    Ok(Prepared {
        pid,
        window_number,
        layers,
        events,
        degradations,
    })
}

#[cfg(test)]
#[path = "mouse_background_tests.rs"]
mod tests;
