use super::{ActionabilityPreflight, ResolvedRefAction};
use crate::cursor_overlay::CursorPhase;
use crate::{Action, CommandContext, PlatformAdapter};

pub(super) fn before_dispatch(
    target: &ResolvedRefAction<'_>,
    preflight: &ActionabilityPreflight,
    lease: &crate::InteractionLease,
) {
    let Some(destination) = preflight.presentation_point.clone() else {
        return;
    };
    crate::cursor_overlay::submit_travel(target.adapter, target.context, destination, lease);
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
    let Some(destination) = preflight.presentation_point.clone() else {
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

fn is_click(action: &Action) -> bool {
    matches!(
        action,
        Action::Click | Action::DoubleClick | Action::RightClick | Action::TripleClick
    )
}
