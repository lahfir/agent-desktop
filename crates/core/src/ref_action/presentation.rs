use crate::{
    Action, CommandContext, DeliveryDisposition, DeliverySemantics, PlatformAdapter, Point,
};

pub(super) fn after_dispatch(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Option<Point>,
    action: &Action,
    result: &Result<crate::ActionResult, crate::AdapterError>,
) {
    let disposition = match result {
        Ok(result) => result.disposition(),
        Err(error) => error.disposition,
    };
    if result.is_err() && !confirms_dispatch(disposition) {
        return;
    }
    let Some(destination) = destination else {
        return;
    };
    let click = is_click(action) && confirms_dispatch(disposition);
    crate::cursor_overlay::submit(adapter, context, destination, click);
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
