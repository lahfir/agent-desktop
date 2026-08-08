//! Physical click legs for `DoubleClick`, `TripleClick`, and `RightClick`:
//! headed policy, bounds midpoint, window-foreground verification,
//! elevation gate, then `SendInput` mouse synthesis.

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, InteractionPolicy, MouseButton, MouseEvent, MouseEventKind,
    Point, StepMechanism,
};

use crate::actions::physical_target::{
    delivery_point, ensure_click_delivery_ready, ensure_headed_click_policy,
    focus_lost_before_delivery, read_click_bounds,
};
use crate::input::mouse::synthesize_mouse;
use crate::system::permissions::ensure_budget;
use crate::tree::element::UIAElement;

const CLICK_LABEL: &str = "SendInput.click";

pub(crate) fn double_click_steps(
    element: &UIAElement,
    policy: InteractionPolicy,
    deadline: Deadline,
    verified_point: Option<Point>,
) -> Result<Vec<ActionStep>, AdapterError> {
    physical_click_steps(
        element,
        MouseButton::Left,
        2,
        policy,
        deadline,
        verified_point.as_ref(),
    )
}

pub(crate) fn triple_click_steps(
    element: &UIAElement,
    policy: InteractionPolicy,
    deadline: Deadline,
    verified_point: Option<Point>,
) -> Result<Vec<ActionStep>, AdapterError> {
    physical_click_steps(
        element,
        MouseButton::Left,
        3,
        policy,
        deadline,
        verified_point.as_ref(),
    )
}

pub(crate) fn right_click_steps(
    element: &UIAElement,
    policy: InteractionPolicy,
    deadline: Deadline,
    verified_point: Option<Point>,
) -> Result<Vec<ActionStep>, AdapterError> {
    physical_click_steps(
        element,
        MouseButton::Right,
        1,
        policy,
        deadline,
        verified_point.as_ref(),
    )
}

pub(crate) fn physical_click_steps(
    element: &UIAElement,
    button: MouseButton,
    count: u32,
    policy: InteractionPolicy,
    deadline: Deadline,
    verified_point: Option<&Point>,
) -> Result<Vec<ActionStep>, AdapterError> {
    ensure_headed_click_policy(policy)?;
    ensure_budget(deadline)?;
    let foreground_ready = ensure_click_delivery_ready(element, deadline)?;
    let bounds = read_click_bounds(element)?;
    click_from_gate(
        bounds,
        verified_point,
        foreground_ready,
        button,
        count,
        deadline,
    )
    .map(|step| vec![step])
}

pub(crate) fn click_from_gate(
    bounds: agent_desktop_core::Rect,
    verified_point: Option<&Point>,
    foreground_ready: bool,
    button: MouseButton,
    count: u32,
    deadline: Deadline,
) -> Result<ActionStep, AdapterError> {
    if !foreground_ready {
        return Err(focus_lost_before_delivery());
    }
    let point = delivery_point(bounds, verified_point)?;
    point.validate()?;
    synthesize_mouse(
        MouseEvent {
            kind: MouseEventKind::Click { count },
            point,
            button,
            modifiers: Vec::new(),
        },
        deadline,
    )?;
    Ok(ActionStep::succeeded(CLICK_LABEL)
        .with_mechanism(StepMechanism::PhysicalSynthetic)
        .with_verified(false))
}

#[cfg(all(test, target_os = "windows"))]
#[path = "physical_click_tests.rs"]
mod tests;
