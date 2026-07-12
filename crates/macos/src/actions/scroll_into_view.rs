#[cfg(target_os = "macos")]
mod imp {
    use crate::actions::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;
    use agent_desktop_core::{
        AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode, InteractionPolicy, Rect,
    };
    use std::time::{Duration, Instant};

    const MAX_ANCESTOR_SCROLLS: usize = 10;

    pub fn scroll_into_view_impl(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        scroll_into_view_outcome(element, deadline).map(|_| ())
    }

    pub(crate) fn scroll_into_view_outcome(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        match scroll_to_verified(element, deadline)? {
            outcome @ (DeliveryOutcome::SatisfiedNoDelivery
            | DeliveryOutcome::DeliveredVerified) => Ok(outcome),
            DeliveryOutcome::NotDelivered => scroll_ancestor_until_visible(element, deadline),
            DeliveryOutcome::DeliveredUnverified => Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "AXScrollToVisible completed without verified visibility",
            )
            .with_disposition(DeliverySemantics::delivered_unverified())),
        }
    }

    fn scroll_ancestor_until_visible(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        for attempt in 0..MAX_ANCESTOR_SCROLLS {
            let Some(direction) = direction_to_window(element, deadline)? else {
                return Ok(if attempt == 0 {
                    DeliveryOutcome::SatisfiedNoDelivery
                } else {
                    DeliveryOutcome::DeliveredVerified
                });
            };
            crate::actions::scroll::ax_scroll(
                element,
                &direction,
                1,
                InteractionPolicy::headless(),
                deadline,
            )?;
        }
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "Semantic ancestor scrolling did not bring the target into view",
        )
        .with_disposition(DeliverySemantics::delivered_unverified()))
    }

    fn direction_to_window(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<Option<Direction>, AdapterError> {
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        let bounds = crate::tree::element_bounds::read_bounds_with_deadline(element, instant)?
            .ok_or_else(|| AdapterError::new(ErrorCode::ActionFailed, "Target has no bounds"))?;
        let window = crate::tree::surface_read::element(element, "AXWindow", instant)?
            .ok_or_else(|| AdapterError::new(ErrorCode::ActionFailed, "Target has no window"))?;
        let window_bounds = crate::tree::element_bounds::read_bounds_with_deadline(
            &window, instant,
        )?
        .ok_or_else(|| AdapterError::new(ErrorCode::ActionFailed, "Target window has no bounds"))?;
        Ok(direction_for_visibility(bounds, window_bounds))
    }

    pub(crate) fn direction_for_visibility(target: Rect, viewport: Rect) -> Option<Direction> {
        if target.y < viewport.y {
            Some(Direction::Up)
        } else if target.y + target.height > viewport.y + viewport.height {
            Some(Direction::Down)
        } else if target.x < viewport.x {
            Some(Direction::Left)
        } else if target.x + target.width > viewport.x + viewport.width {
            Some(Direction::Right)
        } else {
            None
        }
    }

    pub(crate) fn scroll_to_verified(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        prepare(element, deadline)?;
        let before = element_bounds(element, deadline)?;
        if !crate::actions::ax_helpers::try_ax_action_or_err(
            element,
            "AXScrollToVisible",
            deadline,
        )? {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        let local_end = Instant::now() + Duration::from_millis(800);
        loop {
            if visible_in_window(element, deadline).map_err(after_delivery)? {
                return Ok(DeliveryOutcome::DeliveredVerified);
            }
            if deadline.is_expired() {
                return Err(after_delivery(deadline.timeout_error().with_details(
                    serde_json::json!({
                        "verification": "scroll_visibility_not_observed",
                    }),
                )));
            }
            if Instant::now() >= local_end {
                if !scroll_effect_observed(before, element_bounds(element, deadline)?) {
                    return Ok(DeliveryOutcome::NotDelivered);
                }
                return Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    "AXScrollToVisible completed but target visibility was not verified",
                )
                .with_disposition(DeliverySemantics::delivered_unverified()));
            }
            let pause = deadline
                .remaining_slice(Duration::from_millis(20))
                .map_err(after_delivery)?;
            std::thread::sleep(pause.min(Duration::from_millis(20)));
        }
    }

    fn element_bounds(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        crate::tree::element_bounds::read_bounds_with_deadline(
            element,
            crate::tree::locator_deadline::from_operation(deadline)?,
        )
    }

    pub(crate) fn scroll_effect_observed(before: Option<Rect>, after: Option<Rect>) -> bool {
        match (before, after) {
            (Some(before), Some(after)) => before.bounds_hash() != after.bounds_hash(),
            (None, None) => false,
            _ => true,
        }
    }

    fn visible_in_window(element: &AXElement, deadline: Deadline) -> Result<bool, AdapterError> {
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        let Some(bounds) =
            crate::tree::element_bounds::read_bounds_with_deadline(element, instant)?
        else {
            return Ok(false);
        };
        let Some(window) = crate::tree::surface_read::element(element, "AXWindow", instant)? else {
            return Ok(false);
        };
        let Some(window_bounds) =
            crate::tree::element_bounds::read_bounds_with_deadline(&window, instant)?
        else {
            return Ok(false);
        };
        Ok(rect_has_area(bounds) && intersects(bounds, window_bounds))
    }

    pub(crate) fn rect_has_area(rect: Rect) -> bool {
        rect.x.is_finite()
            && rect.y.is_finite()
            && rect.width.is_finite()
            && rect.height.is_finite()
            && rect.width > 0.0
            && rect.height > 0.0
    }

    pub(crate) fn intersects(left: Rect, right: Rect) -> bool {
        left.x < right.x + right.width
            && left.x + left.width > right.x
            && left.y < right.y + right.height
            && left.y + left.height > right.y
    }

    fn prepare(element: &AXElement, deadline: Deadline) -> Result<(), AdapterError> {
        crate::tree::attributes::set_messaging_timeout(element, deadline)
    }

    fn after_delivery(error: AdapterError) -> AdapterError {
        error.with_disposition(DeliverySemantics::delivered_unverified())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::AXElement;
    use agent_desktop_core::{AdapterError, Deadline};

    pub fn scroll_into_view_impl(
        _element: &AXElement,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("scroll_into_view"))
    }
}

pub(crate) use imp::scroll_into_view_impl;

#[cfg(target_os = "macos")]
pub(crate) use imp::scroll_into_view_outcome;

#[cfg(all(test, target_os = "macos"))]
pub(crate) use imp::{direction_for_visibility, intersects, rect_has_area, scroll_effect_observed};

#[cfg(all(test, target_os = "macos"))]
#[path = "scroll_into_view_tests.rs"]
mod tests;
