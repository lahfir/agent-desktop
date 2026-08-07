//! ScrollPattern directional scroll with per-axis honesty (A19-8).
//!
//! Gates `ScrollAvailable` on the element, maps direction to
//! SmallIncrement/SmallDecrement on one axis, invokes `amount` times under the
//! deadline, and verifies by scroll-percent delta or bounds change.

use agent_desktop_core::{
    ActionStep, AdapterError, Deadline, Direction, ErrorCode, InteractionPolicy, Rect,
};

use crate::actions::chain::{DeliveryOutcome, build_step};
use crate::tree::element::UIAElement;

pub(crate) const SCROLL_LABEL: &str = "ScrollPattern.Scroll";

const MAX_SCROLL_AMOUNT: u32 = 1_000;

/// Injected scroll plan shared by the live path and unit-test seam.
pub(crate) struct ScrollPlan {
    pub(crate) scroll_available: bool,
    pub(crate) axis_scrollable: bool,
    pub(crate) axis_name: &'static str,
    pub(crate) amount: u32,
}

pub(crate) fn axis_name(direction: &Direction) -> &'static str {
    match direction {
        Direction::Up | Direction::Down => "vertical",
        Direction::Left | Direction::Right => "horizontal",
    }
}

/// Percent-delta wins when both samples exist; otherwise bounds change.
pub(crate) fn scroll_effect_verified(
    before_percent: Option<f64>,
    after_percent: Option<f64>,
    before_bounds: Option<Rect>,
    after_bounds: Option<Rect>,
) -> bool {
    match (before_percent, after_percent) {
        (Some(before), Some(after)) if (after - before).abs() > f64::EPSILON => true,
        _ => match (before_bounds, after_bounds) {
            (Some(before), Some(after)) => before.bounds_hash() != after.bounds_hash(),
            _ => false,
        },
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        MAX_SCROLL_AMOUNT, SCROLL_LABEL, ActionStep, AdapterError, Deadline, DeliveryOutcome,
        Direction, ErrorCode, InteractionPolicy, ScrollPlan, UIAElement, axis_name, build_step,
        scroll_effect_verified,
    };
    use crate::actions::mutation::{classify_mutation, classify_success};
    use crate::system::permissions::ensure_budget;
    use crate::tree::automation::{ERR_NONE, UiaFailure, failure_of};
    use crate::tree::properties::read_one;
    use crate::tree::property_ids::TreeProperty;
    use agent_desktop_core::{DeliverySemantics, LocatorField, Rect};
    use uiautomation::patterns::UIScrollPattern;
    use uiautomation::types::ScrollAmount;

    pub(crate) fn scroll_steps(
        element: &UIAElement,
        direction: &Direction,
        amount: u32,
        _policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        validate_amount(amount)?;
        let scroll_available =
            read_one(element, TreeProperty::ScrollAvailable).flag() == Some(true);
        let pattern = if scroll_available {
            element.0.get_pattern::<UIScrollPattern>().ok()
        } else {
            None
        };
        let axis = axis_name(direction);
        let axis_scrollable = pattern
            .as_ref()
            .map(|pattern| axis_is_scrollable(pattern, direction))
            .unwrap_or(false);
        let before_percent = pattern
            .as_ref()
            .and_then(|pattern| read_percent(pattern, direction));
        let before_bounds = read_bounds(element);
        let (horizontal, vertical) = scroll_amounts(direction);
        let mut scroll_once = || {
            let Some(pattern) = pattern.as_ref() else {
                return Ok(());
            };
            let delivered = match pattern.scroll(horizontal, vertical) {
                Ok(()) => classify_success()?,
                Err(error) => classify_write("Scroll", SCROLL_LABEL, &error)?,
            };
            if delivered {
                Ok(())
            } else {
                Err(axis_not_scrollable(axis))
            }
        };
        let mut observe = || {
            let after_percent = pattern
                .as_ref()
                .and_then(|pattern| read_percent(pattern, direction));
            let after_bounds = read_bounds(element);
            scroll_effect_verified(before_percent, after_percent, before_bounds, after_bounds)
        };
        scroll_judged_for(
            deadline,
            ScrollPlan {
                scroll_available,
                axis_scrollable,
                axis_name: axis,
                amount,
            },
            &mut scroll_once,
            &mut observe,
        )
    }

    pub(crate) fn scroll_judged_for(
        deadline: Deadline,
        plan: ScrollPlan,
        scroll_once: &mut dyn FnMut() -> Result<(), AdapterError>,
        observe_effect: &mut dyn FnMut() -> bool,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        ensure_budget(deadline)?;
        validate_amount(plan.amount)?;
        if !plan.scroll_available {
            return Err(scroll_unavailable());
        }
        if !plan.axis_scrollable {
            return Err(axis_not_scrollable(plan.axis_name));
        }
        for completed in 0..plan.amount {
            ensure_budget(deadline).map_err(|error| {
                if completed == 0 {
                    error
                } else {
                    partial_scroll_error(completed, plan.amount)
                }
            })?;
            scroll_once().map_err(|error| {
                if completed == 0 {
                    error
                } else {
                    partial_scroll_error(completed, plan.amount)
                }
            })?;
        }
        let verified = observe_effect();
        Ok(vec![build_step(
            SCROLL_LABEL,
            DeliveryOutcome::from_delivery(true, verified),
        )])
    }

    fn scroll_amounts(direction: &Direction) -> (ScrollAmount, ScrollAmount) {
        match direction {
            Direction::Down => (ScrollAmount::NoAmount, ScrollAmount::SmallIncrement),
            Direction::Up => (ScrollAmount::NoAmount, ScrollAmount::SmallDecrement),
            Direction::Right => (ScrollAmount::SmallIncrement, ScrollAmount::NoAmount),
            Direction::Left => (ScrollAmount::SmallDecrement, ScrollAmount::NoAmount),
        }
    }

    fn axis_is_scrollable(pattern: &UIScrollPattern, direction: &Direction) -> bool {
        match direction {
            Direction::Up | Direction::Down => pattern.is_vertically_scrollable().unwrap_or(false),
            Direction::Left | Direction::Right => {
                pattern.is_horizontally_scrollable().unwrap_or(false)
            }
        }
    }

    fn read_percent(pattern: &UIScrollPattern, direction: &Direction) -> Option<f64> {
        match direction {
            Direction::Up | Direction::Down => pattern.get_vertical_scroll_percent().ok(),
            Direction::Left | Direction::Right => pattern.get_horizontal_scroll_percent().ok(),
        }
    }

    fn read_bounds(element: &UIAElement) -> Option<Rect> {
        match read_one(element, TreeProperty::BoundingRectangle).bounds() {
            LocatorField::Known(bounds) => Some(bounds),
            _ => None,
        }
    }

    fn validate_amount(amount: u32) -> Result<(), AdapterError> {
        if amount == 0 || amount > MAX_SCROLL_AMOUNT {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "Scroll amount must be between 1 and 1000",
            ));
        }
        Ok(())
    }

    fn scroll_unavailable() -> AdapterError {
        AdapterError::new(
            ErrorCode::ActionFailed,
            "ScrollPattern is not available on this element",
        )
        .with_disposition(DeliverySemantics::not_delivered())
        .with_suggestion("Target an element that advertises Scroll, or scroll a containing viewport.")
    }

    fn axis_not_scrollable(axis: &str) -> AdapterError {
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!("The {axis} axis is not scrollable"),
        )
        .with_disposition(DeliverySemantics::not_delivered())
        .with_details(serde_json::json!({
            "kind": "axis_not_scrollable",
            "axis": axis,
        }))
        .with_suggestion("Scroll along the axis the control exposes, or target a different container.")
    }

    fn partial_scroll_error(completed: u32, requested: u32) -> AdapterError {
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!("ScrollPattern.Scroll stopped after {completed} of {requested} requested scroll steps"),
        )
        .with_details(serde_json::json!({
            "action_may_have_completed": true,
            "completed_steps": completed,
            "requested_steps": requested,
        }))
        .with_disposition(DeliverySemantics::delivered_unverified())
        .with_suggestion("Inspect the current scroll position before deciding whether to retry.")
    }

    fn classify_write(
        operation: &str,
        api: &str,
        error: &uiautomation::Error,
    ) -> Result<bool, AdapterError> {
        match failure_of(error) {
            UiaFailure::Sentinel(ERR_NONE) => Ok(false),
            other if other.is_exhaustion() => Ok(false),
            failure => classify_mutation(operation, api, &failure),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionStep, AdapterError, Deadline, Direction, InteractionPolicy, ScrollPlan, UIAElement};

    pub(crate) fn scroll_steps(
        _element: &UIAElement,
        _direction: &Direction,
        _amount: u32,
        _policy: InteractionPolicy,
        _deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Scroll"))
    }

    pub(crate) fn scroll_judged_for(
        _deadline: Deadline,
        _plan: ScrollPlan,
        _scroll_once: &mut dyn FnMut() -> Result<(), AdapterError>,
        _observe_effect: &mut dyn FnMut() -> bool,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Scroll"))
    }
}

pub(crate) use imp::scroll_steps;

#[cfg(all(test, target_os = "windows"))]
pub(crate) use imp::scroll_judged_for;

#[cfg(all(test, target_os = "windows"))]
#[path = "scroll_tests.rs"]
mod tests;
