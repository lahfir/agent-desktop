//! Ancestor-scroll ladder for ScrollTo when ScrollItemPattern is absent or
//! leaves geometry unchanged (A19-7).
//!
//! Direction comes from the target's fresh bounds against the nearest
//! ScrollPattern-available ancestor viewport (macOS `direction_for_visibility`
//! parity). Visibility uses the shipped ScrollIntoView predicate — IsOffscreen
//! false, positive-area bounds, and viewport intersection — because provider
//! rects are unclipped (A18-2).

use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Direction, ErrorCode, Rect};

use crate::actions::chain::DeliveryOutcome;
use crate::system::permissions::ensure_budget;
use crate::tree::element::UIAElement;

pub(crate) const MAX_ANCESTOR_SCROLLS: usize = 10;
pub(crate) const LADDER_SCROLL_LABEL: &str = "ScrollPattern.Scroll";

/// Pure geometry → scroll direction (vertical before horizontal, before-edge
/// before after-edge).
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

/// Injected ladder — unit-test seam and live path.
///
/// `next_direction` returns `None` when the target is in view (live path folds
/// the intersection predicate into that signal so an unclipped straddler does
/// not verify — A18-2). `scroll_once` fires one SmallIncrement write.
pub(crate) fn ladder_judged_for(
    deadline: Deadline,
    next_direction: &mut dyn FnMut() -> Result<Option<Direction>, AdapterError>,
    scroll_once: &mut dyn FnMut(&Direction) -> Result<(), AdapterError>,
) -> Result<DeliveryOutcome, AdapterError> {
    let mut scrolled = false;
    for attempt in 0..MAX_ANCESTOR_SCROLLS {
        ensure_budget(deadline).map_err(|error| budget_disposition(scrolled, error))?;
        let Some(direction) =
            next_direction().map_err(|error| budget_disposition(scrolled, error))?
        else {
            return Ok(if attempt == 0 {
                DeliveryOutcome::SatisfiedNoDelivery
            } else {
                DeliveryOutcome::DeliveredVerified
            });
        };
        scroll_once(&direction)?;
        scrolled = true;
    }
    Err(AdapterError::new(
        ErrorCode::ActionFailed,
        "Semantic ancestor scrolling did not bring the target into view",
    )
    .with_disposition(DeliverySemantics::delivered_unverified()))
}

fn budget_disposition(scrolled: bool, error: AdapterError) -> AdapterError {
    if scrolled {
        error.with_disposition(DeliverySemantics::delivered_unverified())
    } else {
        error.with_disposition(DeliverySemantics::not_delivered())
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        AdapterError, Deadline, DeliveryOutcome, Direction, LADDER_SCROLL_LABEL, UIAElement,
        direction_for_visibility, ladder_judged_for,
    };
    use crate::actions::mutation::{classify_mutation, classify_success};
    use crate::system::permissions::ensure_budget;
    use crate::tree::automation::{ERR_NONE, UiaFailure, automation_client, failure_of};
    use crate::tree::live_read::corroborate_verified_process;
    use crate::tree::properties::{read_one, rect_has_area};
    use crate::tree::property_ids::TreeProperty;
    use crate::tree::property_outcome::{PropertyOutcome, PropertyValue};
    use crate::tree::walker_source::{nearest_scroll_viewport, viewport_bounds};
    use agent_desktop_core::{ErrorCode, LocatorField, Rect};
    use uiautomation::core::UITreeWalker;
    use uiautomation::patterns::UIScrollPattern;
    use uiautomation::types::ScrollAmount;

    struct VisibilitySample {
        bounds: Option<Rect>,
        offscreen: Option<bool>,
        viewport: Option<Rect>,
    }

    /// On-screen, positive area, viewport intersection (A18-2).
    fn visibility_ok(sample: &VisibilitySample) -> bool {
        let Some(bounds) = sample.bounds else {
            return false;
        };
        if !rect_has_area(&bounds) {
            return false;
        }
        if sample.offscreen != Some(false) {
            return false;
        }
        match sample.viewport {
            Some(viewport) => {
                bounds.x < viewport.x + viewport.width
                    && bounds.x + bounds.width > viewport.x
                    && bounds.y < viewport.y + viewport.height
                    && bounds.y + bounds.height > viewport.y
            }
            None => false,
        }
    }

    /// Runs the ancestor ladder when a scrollable ancestor exists.
    ///
    /// `Ok(None)` means no ancestor — the caller keeps its terminal arm.
    pub(crate) fn ancestor_ladder(
        element: &UIAElement,
        deadline: Deadline,
    ) -> Result<Option<DeliveryOutcome>, AdapterError> {
        ensure_budget(deadline)?;
        corroborate_verified_process(element)?;
        let client = automation_client()?;
        let Some(walker) = client.get_raw_view_walker().ok() else {
            return Ok(None);
        };
        match nearest_scroll_viewport(element, &walker, deadline) {
            Ok(None) => Ok(None),
            Err(crate::tree::walker_source::BudgetExpired) => Err(deadline.timeout_error()),
            Ok(Some(_)) => scroll_ancestor_until_visible(element, &walker, deadline).map(Some),
        }
    }

    fn scroll_ancestor_until_visible(
        element: &UIAElement,
        walker: &UITreeWalker,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let mut next_direction = || direction_when_not_visible(element, walker, deadline);
        let mut scroll_once =
            |direction: &Direction| scroll_nearest_ancestor(element, walker, direction, deadline);
        ladder_judged_for(deadline, &mut next_direction, &mut scroll_once)
    }

    /// `None` only when the ScrollIntoView visibility predicate passes (A18-2).
    fn direction_when_not_visible(
        element: &UIAElement,
        walker: &UITreeWalker,
        deadline: Deadline,
    ) -> Result<Option<Direction>, AdapterError> {
        let sample = observe_visibility(element, walker, deadline)?;
        if visibility_ok(&sample) {
            return Ok(None);
        }
        let (Some(bounds), Some(viewport)) = (sample.bounds, sample.viewport) else {
            return Ok(Some(Direction::Down));
        };
        Ok(Some(
            direction_for_visibility(bounds, viewport).unwrap_or(Direction::Down),
        ))
    }

    fn observe_visibility(
        element: &UIAElement,
        walker: &UITreeWalker,
        deadline: Deadline,
    ) -> Result<VisibilitySample, AdapterError> {
        ensure_budget(deadline)?;
        corroborate_verified_process(element)?;
        let bounds = match read_one(element, TreeProperty::BoundingRectangle).bounds() {
            LocatorField::Known(bounds) => Some(bounds),
            LocatorField::Absent => None,
            LocatorField::Unknown => {
                return Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    "Could not re-read target bounds during ancestor scroll",
                ));
            }
        };
        let offscreen = match read_one(element, TreeProperty::IsOffscreen) {
            PropertyOutcome::Known(PropertyValue::Flag(flag)) => Some(flag),
            PropertyOutcome::Absent => None,
            PropertyOutcome::Unknown => {
                return Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    "Could not re-read IsOffscreen during ancestor scroll",
                ));
            }
            PropertyOutcome::Known(_) => None,
        };
        let viewport = match nearest_scroll_viewport(element, walker, deadline) {
            Ok(viewport) => viewport.as_ref().and_then(viewport_bounds),
            Err(crate::tree::walker_source::BudgetExpired) => {
                return Err(deadline.timeout_error());
            }
        };
        Ok(VisibilitySample {
            bounds,
            offscreen,
            viewport,
        })
    }

    fn scroll_nearest_ancestor(
        element: &UIAElement,
        walker: &UITreeWalker,
        direction: &Direction,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        ensure_budget(deadline)?;
        let ancestor = match nearest_scroll_viewport(element, walker, deadline) {
            Ok(Some(ancestor)) => ancestor,
            Ok(None) => {
                return Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    "Scrollable ancestor disappeared during ancestor scroll",
                ));
            }
            Err(crate::tree::walker_source::BudgetExpired) => {
                return Err(deadline.timeout_error());
            }
        };
        corroborate_verified_process(&ancestor)?;
        let pattern = match ancestor.0.get_pattern::<UIScrollPattern>() {
            Ok(pattern) => pattern,
            Err(error) => match failure_of(&error) {
                UiaFailure::Sentinel(ERR_NONE) => return Ok(()),
                other if other.is_exhaustion() => return Ok(()),
                failure => {
                    classify_mutation("Scroll", LADDER_SCROLL_LABEL, &failure)?;
                    return Ok(());
                }
            },
        };
        let (horizontal, vertical) = scroll_amounts(direction);
        match pattern.scroll(horizontal, vertical) {
            Ok(()) => {
                classify_success()?;
                Ok(())
            }
            Err(error) => match failure_of(&error) {
                UiaFailure::Sentinel(ERR_NONE) => Ok(()),
                other if other.is_exhaustion() => Ok(()),
                failure => {
                    classify_mutation("Scroll", LADDER_SCROLL_LABEL, &failure)?;
                    Ok(())
                }
            },
        }
    }

    fn scroll_amounts(direction: &Direction) -> (ScrollAmount, ScrollAmount) {
        match direction {
            Direction::Down => (ScrollAmount::NoAmount, ScrollAmount::SmallIncrement),
            Direction::Up => (ScrollAmount::NoAmount, ScrollAmount::SmallDecrement),
            Direction::Right => (ScrollAmount::SmallIncrement, ScrollAmount::NoAmount),
            Direction::Left => (ScrollAmount::SmallDecrement, ScrollAmount::NoAmount),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{AdapterError, Deadline, DeliveryOutcome, UIAElement};

    pub(crate) fn ancestor_ladder(
        _element: &UIAElement,
        _deadline: Deadline,
    ) -> Result<Option<DeliveryOutcome>, AdapterError> {
        Ok(None)
    }
}

pub(crate) use imp::ancestor_ladder;

/// Ladder seam: a scrollable ancestor replaces the unsupported / not-delivered
/// terminal; no ancestor keeps that terminal byte-identical.
pub(crate) fn apply_ladder_seam(
    fallback: AdapterError,
    ladder: Result<Option<DeliveryOutcome>, AdapterError>,
) -> Result<DeliveryOutcome, AdapterError> {
    match ladder {
        Ok(Some(outcome)) => Ok(outcome),
        Ok(None) => Err(fallback),
        Err(error) => Err(error),
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "scroll_ladder_tests.rs"]
mod tests;
