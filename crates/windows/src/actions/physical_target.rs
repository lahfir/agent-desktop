//! Shared helpers for ref-addressed physical input: process identity,
//! elevation gate, window foreground, and bounds-derived delivery points.

use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, InteractionPolicy, Point, ProcessId, Rect,
};

use crate::input::elevation::ensure_target_integrity_allows_input;
use crate::system::permissions::ensure_budget;
use crate::system::window_ops::is_root_foreground_window;
use crate::tree::element::UIAElement;
use crate::tree::live_read::corroborate_verified_process;
use crate::tree::properties::read_one;
use crate::tree::property_ids::TreeProperty;

pub(crate) fn target_process_id(element: &UIAElement) -> Result<ProcessId, AdapterError> {
    if let Some((pid, _)) = element.verified_process() {
        return Ok(ProcessId::new(pid));
    }
    element
        .0
        .get_process_id()
        .ok()
        .filter(|&pid| pid != 0)
        .map(ProcessId::new)
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::StaleRef,
                "Physical input target no longer has a valid owning process",
            )
        })
}

pub(crate) fn ensure_elevation_allows(element: &UIAElement) -> Result<(), AdapterError> {
    ensure_target_integrity_allows_input(target_process_id(element)?)
}

pub(crate) fn ensure_keyboard_policy(policy: InteractionPolicy) -> Result<(), AdapterError> {
    if !policy.allow_focus_steal {
        return Err(AdapterError::policy_denied_for_policy(
            "Physical keyboard fallback requires focus permission",
            policy,
        ));
    }
    Ok(())
}

pub(crate) fn ensure_headed_click_policy(policy: InteractionPolicy) -> Result<(), AdapterError> {
    if !policy.allow_cursor_move || !policy.allow_focus_steal {
        return Err(AdapterError::policy_denied_for_policy(
            "Physical click fallback is disabled by the current interaction policy",
            policy,
        ));
    }
    Ok(())
}

pub(crate) fn ensure_click_delivery_ready(
    element: &UIAElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    ensure_budget(deadline)?;
    corroborate_verified_process(element)?;
    ensure_elevation_allows(element)?;
    Ok(target_window_is_foreground(element, deadline))
}

pub(crate) fn ensure_keyboard_delivery_ready(
    element: &UIAElement,
    deadline: Deadline,
) -> Result<bool, AdapterError> {
    ensure_budget(deadline)?;
    corroborate_verified_process(element)?;
    ensure_elevation_allows(element)?;
    crate::actions::focus::keyboard_focus_ready(element)
}

pub(crate) fn delivery_point(
    bounds: Rect,
    verified: Option<&Point>,
) -> Result<Point, AdapterError> {
    let point = verified.cloned().unwrap_or(Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    });
    if point.x < bounds.x
        || point.x > bounds.x + bounds.width
        || point.y < bounds.y
        || point.y > bounds.y + bounds.height
    {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "Actionability-verified input point is outside the target's live bounds",
        )
        .with_suggestion("Run 'snapshot' to refresh, then retry with the updated ref.")
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    Ok(point)
}

pub(crate) fn read_click_bounds(element: &UIAElement) -> Result<Rect, AdapterError> {
    match read_one(element, TreeProperty::BoundingRectangle).bounds() {
        agent_desktop_core::LocatorField::Known(bounds) => {
            if bounds.width <= 0.0 || bounds.height <= 0.0 {
                Err(
                    AdapterError::new(ErrorCode::ActionFailed, "Element has zero-size bounds")
                        .with_suggestion(
                            "Element may be hidden or off-screen. Try 'scroll-to' first.",
                        ),
                )
            } else {
                Ok(bounds)
            }
        }
        agent_desktop_core::LocatorField::Absent => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Element has no readable bounds",
        )
        .with_suggestion("Physical click requires readable bounds")),
        agent_desktop_core::LocatorField::Unknown => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Could not read target bounds for physical click",
        )),
    }
}

pub(crate) fn focus_lost_before_delivery() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        "Target element lost focus before physical input delivery",
    )
    .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
}

/// Whether the window hosting `element` owns the desktop foreground.
///
/// UIA reports `NativeWindowHandle` 0 - success, not failure - for every
/// element that is not itself a window, which is the normal shape for WPF,
/// WinUI, UWP and Chromium content. Reading the leaf's handle alone would
/// therefore answer "no window, not foreground" for most modern UI and
/// refuse delivery before injecting, so the handle is resolved the way the
/// rest of the crate resolves it: climb to the first ancestor that reports a
/// non-zero handle.
fn target_window_is_foreground(element: &UIAElement, deadline: Deadline) -> bool {
    host_window_handle(element, deadline).is_some_and(is_root_foreground_window)
}

#[cfg(target_os = "windows")]
fn host_window_handle(
    element: &UIAElement,
    deadline: Deadline,
) -> Option<crate::system::window_enum::WindowHandle> {
    let client = crate::tree::automation::automation_client().ok()?;
    let walker = client.get_raw_view_walker().ok()?;
    let hwnd = crate::tree::hit_test::corroborate::first_native_hwnd(element, &walker, deadline)?;
    Some(hwnd as crate::system::window_enum::WindowHandle)
}

#[cfg(not(target_os = "windows"))]
fn host_window_handle(
    element: &UIAElement,
    deadline: Deadline,
) -> Option<crate::system::window_enum::WindowHandle> {
    let _ = (element, deadline);
    None
}

/// Exposes the host-window climb to the live lane, which is the only place a
/// real element with a zero leaf handle exists.
#[cfg(test)]
pub(crate) fn host_window_handle_for_test(
    element: &UIAElement,
    deadline: Deadline,
) -> Option<crate::system::window_enum::WindowHandle> {
    host_window_handle(element, deadline)
}
