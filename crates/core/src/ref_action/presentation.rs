use super::ActionabilityPreflight;
use crate::{Action, CommandContext, DeliveryDisposition, DeliverySemantics, PlatformAdapter};

pub(super) fn before_dispatch(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    preflight: &ActionabilityPreflight,
) {
    let Some(destination) = preflight.presentation_point.clone() else {
        return;
    };
    crate::cursor_overlay::submit(adapter, context, destination, None, false);
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
    if !is_click(action) || !confirms_dispatch(disposition) {
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
        true,
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
