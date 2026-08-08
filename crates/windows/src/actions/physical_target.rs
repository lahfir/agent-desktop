//! Shared helpers for ref-addressed physical input: process identity,
//! elevation gate, window foreground, and bounds-derived delivery points.

use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, InteractionPolicy, Point, ProcessId, Rect,
};

use crate::input::elevation::ensure_target_integrity_allows_input;
use crate::system::permissions::ensure_budget;
use crate::system::window_ops::is_foreground_window;
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
    Ok(target_window_is_foreground(element))
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
        return Err(AdapterError::stale_ref(
            "Actionability-verified input point is outside the target's live bounds",
        ));
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

fn target_window_is_foreground(element: &UIAElement) -> bool {
    element
        .0
        .get_native_window_handle()
        .ok()
        .map(|handle| {
            use windows::Win32::Foundation::HWND;
            let hwnd: HWND = handle.into();
            is_foreground_window(hwnd.0)
        })
        .unwrap_or(false)
}
