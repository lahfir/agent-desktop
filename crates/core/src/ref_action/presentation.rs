use std::cell::Cell;

use super::{ActionabilityPreflight, ResolvedRefAction};
use crate::cursor_overlay::CursorPhase;
use crate::{Action, CommandContext, PlatformAdapter};

const SAME_POINT: f64 = 1.5;

thread_local! {
    static TRAVELLED_BEFORE_LEASE: Cell<Option<(f64, f64)>> = const { Cell::new(None) };
}

pub(super) fn before_dispatch(
    target: &ResolvedRefAction<'_>,
    preflight: &ActionabilityPreflight,
    action: &Action,
    lease: &crate::InteractionLease,
) {
    let Some(destination) = destination(preflight) else {
        return;
    };
    let travelled = TRAVELLED_BEFORE_LEASE.get();
    if travelled.is_some_and(|(x, y)| (destination.x - x).hypot(destination.y - y) < SAME_POINT) {
        return;
    }
    TRAVELLED_BEFORE_LEASE.set(None);
    let pointer = shows_pointer(&target.entry.identity.role, action);
    crate::cursor_overlay::submit_travel(
        target.adapter,
        target.context,
        destination,
        lease,
        pointer,
    );
}

/// Starts the drawn cursor's travel toward the target before the interaction
/// lease is taken, from a read-only resolution of the element. Whatever this
/// observes is re-resolved under the lease before dispatch, so it only decides
/// where the cursor heads. Headed actions keep their travel under the lease,
/// because they move the real pointer.
pub(super) fn before_lease(
    context: super::ref_action_wait_context::RefActionWaitContext<'_>,
    request: &crate::ActionRequest,
    deadline: crate::Deadline,
) {
    TRAVELLED_BEFORE_LEASE.set(None);
    if request.policy.is_headed() || !context.context.cursor_overlay().is_enabled() {
        return;
    }
    let Some(destination) = target_center(context, deadline) else {
        return;
    };
    let pointer = shows_pointer(&context.entry.identity.role, &request.action);
    if crate::cursor_overlay::submit_travel_before_lease(
        context.adapter,
        context.context,
        destination.clone(),
        deadline,
        pointer,
    ) {
        TRAVELLED_BEFORE_LEASE.set(Some((destination.x, destination.y)));
    }
}

fn target_center(
    context: super::ref_action_wait_context::RefActionWaitContext<'_>,
    deadline: crate::Deadline,
) -> Option<crate::Point> {
    let geometry = &context.entry.geometry;
    let stored = matches!(
        geometry.bounds_space,
        None | Some(crate::BoundsSpace::Screen)
    )
    .then_some(geometry.bounds.as_ref())
    .flatten()
    .and_then(center_of);
    stored.or_else(|| {
        let handle = context
            .adapter
            .resolve_element_strict(context.entry, deadline)
            .ok()?;
        let bounds = context
            .adapter
            .get_element_bounds(&handle, deadline)
            .ok()
            .flatten()?;
        center_of(&bounds)
    })
}

fn center_of(bounds: &crate::Rect) -> Option<crate::Point> {
    bounds.validate().ok()?;
    (bounds.width > 0.0 && bounds.height > 0.0).then(|| crate::Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    })
}

/// A pressable control shows the pointer however it is reached; any other element shows it
/// only when clicked, and text entry never does.
pub(super) fn shows_pointer(role: &str, action: &Action) -> bool {
    let role = crate::Role::from_token(role);
    role.is_pressable() || (is_click(action) && !role.takes_text())
}

pub(super) fn after_dispatch(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    preflight: &ActionabilityPreflight,
    action: &Action,
    result: &Result<crate::ActionResult, crate::AdapterError>,
) {
    let disposition = match result {
        Ok(result) => result.disposition(),
        Err(error) => error.disposition,
    };
    let travelled = TRAVELLED_BEFORE_LEASE.take();
    if !crate::cursor_overlay::confirms_delivery(disposition) {
        return;
    }
    let Some(destination) = travelled
        .map(|(x, y)| crate::Point { x, y })
        .or_else(|| destination(preflight))
    else {
        return;
    };
    crate::cursor_overlay::submit(
        adapter,
        context,
        destination,
        preflight.presentation_bounds,
        is_click(action),
        CursorPhase::Effect,
    );
}

fn destination(preflight: &ActionabilityPreflight) -> Option<crate::Point> {
    match preflight.pointer_delivery {
        crate::actionability::PointerDelivery::Physical => preflight
            .verified_point
            .clone()
            .or_else(|| preflight.presentation_point.clone()),
        _ => preflight.presentation_point.clone(),
    }
}

fn is_click(action: &Action) -> bool {
    matches!(
        action,
        Action::Click | Action::DoubleClick | Action::RightClick | Action::TripleClick
    )
}

#[cfg(test)]
#[path = "presentation_tests.rs"]
mod tests;
