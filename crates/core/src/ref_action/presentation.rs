use super::{ActionabilityPreflight, ResolvedRefAction};
use crate::cursor_overlay::CursorPhase;
use crate::{Action, CommandContext, DeliveryDisposition, DeliverySemantics, PlatformAdapter};

const DISPATCH_RESERVE_MS: u64 = 100;

pub(super) fn before_dispatch(
    target: &ResolvedRefAction<'_>,
    preflight: &ActionabilityPreflight,
    lease: &crate::InteractionLease,
) {
    if lease.deadline().remaining_ms()
        <= crate::CURSOR_ARRIVAL_TIMEOUT_MS.saturating_add(DISPATCH_RESERVE_MS)
    {
        return;
    }
    let Some(destination) = preflight.presentation_point.clone() else {
        return;
    };
    crate::cursor_overlay::submit(
        target.adapter,
        target.context,
        destination,
        None,
        false,
        CursorPhase::Travel,
    );
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
    if !confirms_dispatch(disposition) {
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

fn confirms_dispatch(disposition: DeliverySemantics) -> bool {
    matches!(
        disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
            | DeliveryDisposition::DeliveredUnverified
            | DeliveryDisposition::DeliveredVerified
    )
}

fn is_click(action: &Action) -> bool {
    matches!(
        action,
        Action::Click | Action::DoubleClick | Action::RightClick | Action::TripleClick
    )
}
