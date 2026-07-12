use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, InteractionPolicy, MouseButton, MouseEvent, MouseEventKind,
    Point,
};

use crate::tree::AXElement;

pub(crate) struct PhysicalClick {
    pub(crate) button: MouseButton,
    pub(crate) count: u32,
    pub(crate) verified_point: Option<Point>,
}

pub(crate) fn click_via_bounds(
    element: &AXElement,
    click: PhysicalClick,
    policy: InteractionPolicy,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    if !policy.allow_cursor_move || !policy.allow_focus_steal {
        return Err(AdapterError::policy_denied_for_policy(
            "Physical click fallback is disabled by the current interaction policy",
            policy,
        ));
    }
    let pid = crate::system::app_ops::pid_from_element(element, deadline).ok_or_else(|| {
        AdapterError::new(
            ErrorCode::StaleRef,
            "Physical click target no longer has an owning application",
        )
    })?;
    let identity =
        crate::system::process_identity::ProcessIdentity::capture(pid)?.ok_or_else(|| {
            AdapterError::new(
                ErrorCode::StaleRef,
                "Physical click target process exited before input preparation",
            )
        })?;
    let window = target_window(element, deadline)?;
    crate::system::focus::ensure_app_focused(pid, deadline)?;
    crate::system::window_ops::raise_window(&window, deadline)?;
    crate::system::focus::verify_app_focused(pid, deadline)?;
    crate::system::focus::verify_window_main(&window, deadline)?;
    let read_deadline = crate::tree::locator_deadline::from_operation(deadline)?;
    let bounds = crate::tree::element_bounds::read_bounds_with_deadline(element, read_deadline)?
        .ok_or_else(|| {
            AdapterError::new(ErrorCode::ActionFailed, "Element has no readable bounds")
                .with_suggestion("AX action failed and CGEvent fallback unavailable")
        })?;
    if bounds.width <= 0.0 || bounds.height <= 0.0 {
        return Err(
            AdapterError::new(ErrorCode::ActionFailed, "Element has zero-size bounds")
                .with_suggestion("Element may be hidden or off-screen. Try 'scroll-to' first."),
        );
    }
    let point = delivery_point(bounds, click.verified_point.as_ref())?;
    crate::input::mouse::validate_point(&point)?;
    tracing::debug!(
        button = ?click.button,
        count = click.count,
        x = point.x,
        y = point.y,
        "AX action failed, falling back to CGEvent click"
    );
    let mut verify_target = || verify_delivery_target(element, &window, &point, identity, deadline);
    crate::input::mouse::synthesize_mouse_after(
        MouseEvent {
            kind: MouseEventKind::Click { count: click.count },
            point: point.clone(),
            button: click.button,
            modifiers: Vec::new(),
        },
        deadline,
        &mut verify_target,
    )
}

fn delivery_point(
    bounds: agent_desktop_core::Rect,
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

fn verify_delivery_target(
    element: &AXElement,
    window: &AXElement,
    point: &Point,
    identity: crate::system::process_identity::ProcessIdentity,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    crate::system::focus::verify_app_focused(identity.pid(), deadline)?;
    crate::system::focus::verify_window_main(window, deadline)?;
    match crate::tree::hit_test::hit_test_ax_element(element, point.clone(), deadline)? {
        agent_desktop_core::hit_test::HitTestResult::ReachesTarget => {}
        agent_desktop_core::hit_test::HitTestResult::InterceptedBy { role, name, .. } => {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Physical click point is intercepted by another accessibility element",
            )
            .with_details(serde_json::json!({
                "physical_delivery_started": false,
                "occluder_role": role,
                "occluder_name": name,
            })));
        }
        agent_desktop_core::hit_test::HitTestResult::Unknown => {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Physical click target could not be proven at the final input point",
            )
            .with_details(serde_json::json!({ "physical_delivery_started": false })));
        }
    }
    if !identity.still_matches()? {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "Physical click target process instance changed at input delivery",
        )
        .with_details(serde_json::json!({ "physical_delivery_started": false })));
    }
    Ok(())
}

fn target_window(element: &AXElement, deadline: Deadline) -> Result<AXElement, AdapterError> {
    crate::tree::attributes::set_messaging_timeout(element, deadline)?;
    let result = crate::tree::attributes::copy_element_attr_result(element, "AXWindow", deadline);
    if deadline.is_expired() {
        return Err(deadline.timeout_error());
    }
    match result {
        Ok(Some(window)) => Ok(window),
        Ok(None) => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Physical click target has no verified owning window",
        )
        .with_details(serde_json::json!({ "physical_delivery_started": false }))),
        Err(error) => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Could not verify the physical click target window",
        )
        .with_details(serde_json::json!({
            "ax_error": error,
            "physical_delivery_started": false,
        }))),
    }
}

#[cfg(test)]
#[path = "physical_click_tests.rs"]
mod tests;
