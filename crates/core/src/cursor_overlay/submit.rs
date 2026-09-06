use super::CursorPhase;
use crate::{
    AdapterError, CommandContext, DeliveryDisposition, DeliverySemantics, PlatformAdapter, Point,
    Rect,
};

const DISPATCH_RESERVE_MS: u64 = 100;

pub(crate) fn submit_travel(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    lease: &crate::InteractionLease,
) {
    if lease.deadline().remaining_ms()
        <= crate::CURSOR_ARRIVAL_TIMEOUT_MS.saturating_add(DISPATCH_RESERVE_MS)
    {
        return;
    }
    let _scope = crate::deadline::enter_scope(Some(lease.deadline().capped(
        std::time::Duration::from_millis(crate::CURSOR_ARRIVAL_TIMEOUT_MS),
    )));
    submit(
        adapter,
        context,
        destination,
        None,
        false,
        CursorPhase::Travel,
    );
}

pub(crate) fn input_was_delivered(result: &Result<(), AdapterError>) -> bool {
    result
        .as_ref()
        .map_or_else(|error| confirms_delivery(error.disposition), |_| true)
}

pub(crate) fn confirms_delivery(disposition: DeliverySemantics) -> bool {
    matches!(
        disposition.delivery(),
        DeliveryDisposition::DeliveryUncertain
            | DeliveryDisposition::DeliveredUnverified
            | DeliveryDisposition::DeliveredVerified
    )
}

pub(crate) fn submit(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    target: Option<Rect>,
    click: bool,
    phase: CursorPhase,
) {
    if !context.cursor_overlay().is_enabled() {
        return;
    }
    let Some(session_id) = context.session_id() else {
        return;
    };
    let instruction =
        match super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), click) {
            Ok(instruction) => instruction.with_target(target).with_phase(phase),
            Err(error) => {
                tracing::warn!(code = %error.code.as_str(), "agent cursor instruction was skipped");
                return;
            }
        };
    let control = super::CursorOverlayControl::present_with_style(
        session_id.to_owned(),
        instruction,
        context.cursor_overlay().style().clone(),
    )
    .with_agent_id(context.agent_id().map(str::to_owned));
    if let Err(error) = adapter.update_cursor_overlay(&control) {
        tracing::warn!(code = %error.code.as_str(), "agent cursor presentation was skipped");
    }
}
