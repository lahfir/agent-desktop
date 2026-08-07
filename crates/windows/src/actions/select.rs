//! SelectionItemPattern select with bounded descendant search (A19-7).
//!
//! Self-match uses SelectionItem on the target; otherwise a walker DFS
//! finds a named SelectionItem. Collapsed ExpandCollapse containers expand
//! first and best-effort collapse after failure. A search miss after expand
//! may scroll-to-realize (A18-1) before ElementNotFound. Container Value
//! verification routes through the IsPassword-gated value helpers.

use agent_desktop_core::{ActionStep, AdapterError, Deadline, ErrorCode};
use std::cell::{Cell, RefCell};
use std::time::{Duration, Instant};

use crate::actions::chain::{DeliveryOutcome, build_step};
use crate::tree::element::UIAElement;

pub(crate) const SELECT_LABEL: &str = "SelectionItemPattern.Select";

const VERIFY_TIMEOUT: Duration = Duration::from_millis(600);
const POLL_SLICE: Duration = Duration::from_millis(25);

/// Pure verification precedence: container Value outranks `is_selected` when
/// the gate produced a comparison; withheld / absent Value falls back.
pub(crate) fn resolve_select_verification(
    container_value: Option<Option<bool>>,
    is_selected: Option<bool>,
) -> Option<bool> {
    match container_value {
        Some(Some(matched)) => Some(matched),
        Some(None) | None => is_selected,
    }
}

/// Injected select plan flags — unit-test seam and live path.
pub(crate) struct SelectPlan {
    pub(crate) self_match: bool,
    pub(crate) needs_expand: bool,
    pub(crate) value_chars: usize,
}

/// Injected select operations — unit-test seam and live path.
pub(crate) struct SelectOps<'a> {
    pub(crate) expand: &'a mut dyn FnMut() -> Result<(), AdapterError>,
    pub(crate) collapse: &'a mut dyn FnMut(),
    pub(crate) find: &'a mut dyn FnMut() -> Result<bool, AdapterError>,
    pub(crate) realize: &'a mut dyn FnMut() -> Result<(), AdapterError>,
    pub(crate) select_item: &'a mut dyn FnMut() -> Result<DeliveryOutcome, AdapterError>,
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        POLL_SLICE, SELECT_LABEL, VERIFY_TIMEOUT, ActionStep, AdapterError, Cell, Deadline,
        DeliveryOutcome, ErrorCode, Instant, RefCell, SelectOps, SelectPlan, UIAElement, build_step,
        resolve_select_verification,
    };
    use crate::actions::disclosure::ExpandKind;
    use crate::actions::mutation::{classify_mutation, classify_success};
    use crate::actions::post_state::after_delivery;
    use crate::actions::select_search::{
        find_named_selection_item, name_matches, scroll_to_realize, selection_item_available,
    };
    use crate::actions::value_write::gated_pattern_value_equals;
    use crate::system::permissions::ensure_budget;
    use crate::tree::automation::{ERR_NONE, UiaFailure, failure_of};
    use crate::tree::properties::read_one;
    use crate::tree::property_ids::TreeProperty;
    use agent_desktop_core::DeliverySemantics;
    use uiautomation::patterns::{UIExpandCollapsePattern, UISelectionItemPattern};

    pub(crate) fn select_steps(
        element: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let self_match = selection_item_available(element) && name_matches(element, value);
        let needs_expand = is_collapsed_container(element);
        let expanded = Cell::new(false);
        let mut expand = || {
            ensure_budget(deadline)?;
            expand_container(element)?;
            expanded.set(true);
            Ok(())
        };
        let mut collapse = || {
            if expanded.get() {
                let _ = collapse_container(element);
            }
        };
        let found_holder = RefCell::new(self_match.then(|| element.clone()));
        let mut find = || {
            if found_holder.borrow().is_some() {
                return Ok(true);
            }
            match find_named_selection_item(element, value, deadline)? {
                Some(found) => {
                    *found_holder.borrow_mut() = Some(found);
                    Ok(true)
                }
                None => Ok(false),
            }
        };
        let mut realize = || {
            ensure_budget(deadline)?;
            scroll_to_realize(element, deadline)
        };
        let mut select_item = || {
            let borrowed = found_holder.borrow();
            let target = borrowed
                .as_ref()
                .ok_or_else(|| not_found_chars(value.chars().count()))?;
            delivered_select(element, target, value, deadline)
        };
        select_judged_for(
            deadline,
            SelectPlan {
                self_match,
                needs_expand,
                value_chars: value.chars().count(),
            },
            SelectOps {
                expand: &mut expand,
                collapse: &mut collapse,
                find: &mut find,
                realize: &mut realize,
                select_item: &mut select_item,
            },
        )
    }

    pub(crate) fn select_judged_for(
        deadline: Deadline,
        plan: SelectPlan,
        ops: SelectOps<'_>,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        ensure_budget(deadline)?;
        if plan.self_match {
            return match (ops.select_item)() {
                Ok(outcome) => Ok(vec![build_step(SELECT_LABEL, outcome)]),
                Err(error) => {
                    (ops.collapse)();
                    Err(error)
                }
            };
        }
        if plan.needs_expand {
            (ops.expand)()?;
        }
        let mut found = (ops.find)().inspect_err(|_| (ops.collapse)())?;
        if !found {
            (ops.realize)().inspect_err(|_| (ops.collapse)())?;
            found = (ops.find)().inspect_err(|_| (ops.collapse)())?;
        }
        if !found {
            (ops.collapse)();
            return Err(not_found_chars(plan.value_chars));
        }
        match (ops.select_item)() {
            Ok(outcome) => Ok(vec![build_step(SELECT_LABEL, outcome)]),
            Err(error) => {
                (ops.collapse)();
                Err(error)
            }
        }
    }

    fn delivered_select(
        container: &UIAElement,
        target: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let delivered = match target.0.get_pattern::<UISelectionItemPattern>() {
            Ok(pattern) => match pattern.select() {
                Ok(()) => classify_success()?,
                Err(error) => classify_write("Select", SELECT_LABEL, &error)?,
            },
            Err(error) => classify_write("get_pattern", SELECT_LABEL, &error)?,
        };
        if !delivered {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        let verified = poll_verified(container, target, value, deadline).map_err(after_delivery)?;
        Ok(DeliveryOutcome::from_observation(verified))
    }

    fn poll_verified(
        container: &UIAElement,
        target: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<Option<bool>, AdapterError> {
        ensure_budget(deadline)?;
        let end = verification_deadline(deadline)?;
        loop {
            let verified = verify_once(container, target, value)?;
            if verified == Some(true) {
                return Ok(Some(true));
            }
            if Instant::now() >= end {
                return Ok(verified);
            }
            std::thread::sleep(deadline.remaining_slice(POLL_SLICE)?);
        }
    }

    fn verify_once(
        container: &UIAElement,
        target: &UIAElement,
        value: &str,
    ) -> Result<Option<bool>, AdapterError> {
        let container_value = if value_available(container) {
            Some(gated_pattern_value_equals(container, value)?)
        } else {
            None
        };
        let selected = read_one(target, TreeProperty::SelectionItemIsSelected).flag();
        Ok(resolve_select_verification(container_value, selected))
    }

    fn expand_container(element: &UIAElement) -> Result<(), AdapterError> {
        match element.0.get_pattern::<UIExpandCollapsePattern>() {
            Ok(pattern) => match pattern.expand() {
                Ok(()) => Ok(()),
                Err(error) => {
                    let _ = classify_write("Expand", "ExpandCollapsePattern.Expand", &error)?;
                    Ok(())
                }
            },
            Err(error) => {
                let _ = classify_write("get_pattern", "ExpandCollapsePattern.Expand", &error)?;
                Ok(())
            }
        }
    }

    fn collapse_container(element: &UIAElement) -> Result<(), AdapterError> {
        match element.0.get_pattern::<UIExpandCollapsePattern>() {
            Ok(pattern) => {
                let _ = pattern.collapse();
                Ok(())
            }
            Err(_) => Ok(()),
        }
    }

    fn is_collapsed_container(element: &UIAElement) -> bool {
        if read_one(element, TreeProperty::ExpandCollapseAvailable).flag() != Some(true) {
            return false;
        }
        matches!(
            read_one(element, TreeProperty::ExpandCollapseState)
                .number()
                .and_then(ExpandKind::from_i32),
            Some(ExpandKind::Collapsed)
        )
    }

    fn value_available(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::ValueAvailable).flag() == Some(true)
    }

    fn verification_deadline(deadline: Deadline) -> Result<Instant, AdapterError> {
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return Err(deadline.timeout_error());
        }
        let local = Instant::now() + VERIFY_TIMEOUT;
        Ok(Instant::now()
            .checked_add(remaining)
            .map_or(local, |cap| cap.min(local)))
    }

    fn not_found_chars(chars: usize) -> AdapterError {
        AdapterError::new(
            ErrorCode::ElementNotFound,
            format!("No selection item matched the requested value ({chars} chars)"),
        )
        .with_disposition(DeliverySemantics::not_delivered())
        .with_suggestion("Use find or snapshot to inspect the available selection items.")
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
    use super::{ActionStep, AdapterError, Deadline, SelectOps, SelectPlan, UIAElement};

    pub(crate) fn select_steps(
        _element: &UIAElement,
        _value: &str,
        _deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Select"))
    }

    pub(crate) fn select_judged_for(
        _deadline: Deadline,
        _plan: SelectPlan,
        _ops: SelectOps<'_>,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Select"))
    }
}

pub(crate) use imp::select_steps;

#[cfg(all(test, target_os = "windows"))]
pub(crate) use imp::select_judged_for;

#[cfg(all(test, target_os = "windows"))]
#[path = "select_tests.rs"]
mod tests;
