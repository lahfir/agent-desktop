use super::{ActionabilityPreflight, ResolvedRefAction};
use crate::cursor_overlay::CursorPhase;
use crate::{Action, CommandContext, PlatformAdapter};

pub(super) fn before_dispatch(
    target: &ResolvedRefAction<'_>,
    preflight: &ActionabilityPreflight,
    lease: &crate::InteractionLease,
) {
    let Some(destination) = destination(preflight) else {
        return;
    };
    let Some(_scope) = crate::cursor_overlay::travel_scope(lease) else {
        return;
    };
    let Some(window) = preflight.presentation_window.clone() else {
        return;
    };
    let instruction =
        crate::CursorOverlayInstruction::new(destination, target.context.cursor_overlay(), false)
            .map(|instruction| instruction.with_window(window));
    crate::cursor_overlay::send(target.adapter, target.context, instruction);
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
    if !crate::cursor_overlay::confirms_delivery(disposition) {
        return;
    }
    let Some(destination) = destination(preflight) else {
        return;
    };
    let Some(window) = preflight.presentation_window.clone() else {
        return;
    };
    let instruction = crate::CursorOverlayInstruction::new(
        destination,
        context.cursor_overlay(),
        is_click(action),
    )
    .map(|instruction| {
        instruction
            .with_window(window)
            .with_target(preflight.presentation_bounds)
            .with_phase(CursorPhase::Effect)
    });
    crate::cursor_overlay::send(adapter, context, instruction);
}

pub(super) fn window(target: &ResolvedRefAction<'_>) -> Option<(crate::ProcessId, String)> {
    crate::cursor_overlay::presentation_window(
        target.adapter,
        target.context,
        target.handle,
        target.entry.process.pid,
        target.deadline,
    )
}

fn destination(preflight: &ActionabilityPreflight) -> Option<crate::Point> {
    match preflight.pointer_delivery {
        crate::actionability::PointerDelivery::Physical => preflight.verified_point.clone(),
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
