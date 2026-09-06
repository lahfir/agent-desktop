use super::CursorPhase;
use crate::{
    AdapterError, CommandContext, DeliveryDisposition, DeliverySemantics, MouseEvent,
    PlatformAdapter, Point, Rect,
};

const DISPATCH_RESERVE_MS: u64 = 100;

pub(crate) fn travel_scope(
    lease: &crate::InteractionLease,
) -> Option<crate::deadline::DeadlineScope> {
    if lease.deadline().remaining_ms()
        <= crate::CURSOR_ARRIVAL_TIMEOUT_MS.saturating_add(DISPATCH_RESERVE_MS)
    {
        return None;
    }
    Some(crate::deadline::enter_scope(Some(lease.deadline().capped(
        std::time::Duration::from_millis(crate::CURSOR_ARRIVAL_TIMEOUT_MS),
    ))))
}

pub(crate) fn submit_travel(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    lease: &crate::InteractionLease,
) {
    let Some(_scope) = travel_scope(lease) else {
        return;
    };
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
    let instruction =
        super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), click)
            .map(|instruction| instruction.with_target(target).with_phase(phase));
    let _ = send(adapter, context, instruction);
}

pub(crate) fn submit_drag(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    drag_from: Point,
    destination: Point,
    lease: &crate::InteractionLease,
) -> bool {
    let Some(_scope) = travel_scope(lease) else {
        return false;
    };
    let instruction =
        super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), false).map(
            |instruction| {
                instruction
                    .with_drag_from(Some(drag_from))
                    .with_phase(CursorPhase::Drag)
            },
        );
    send(adapter, context, instruction)
}

pub(crate) fn submit_drag_effect(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    drag_from: Point,
    destination: Point,
) {
    let instruction =
        super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), false).map(
            |instruction| {
                instruction
                    .with_drag_from(Some(drag_from))
                    .with_phase(CursorPhase::Effect)
            },
        );
    let _ = send(adapter, context, instruction);
}

pub(crate) fn cancel_drag(adapter: &dyn PlatformAdapter, context: &CommandContext) {
    let Ok(cleanup_deadline) = crate::Deadline::detached_after(crate::CURSOR_ARRIVAL_TIMEOUT_MS)
    else {
        return;
    };
    let _scope = crate::deadline::enter_scope(Some(cleanup_deadline));
    let Some(session_id) = enabled_session(context) else {
        return;
    };
    let control = super::CursorOverlayControl::hide(session_id.to_owned())
        .with_agent_id(context.agent_id().map(str::to_owned));
    let _ = update(adapter, &control);
}

fn send(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    instruction: Result<super::CursorOverlayInstruction, AdapterError>,
) -> bool {
    let Some(session_id) = enabled_session(context) else {
        return false;
    };
    let instruction = match instruction {
        Ok(instruction) => instruction,
        Err(error) => {
            tracing::warn!(code = %error.code.as_str(), "agent cursor instruction was skipped");
            return false;
        }
    };
    let control = super::CursorOverlayControl::present_with_style(
        session_id.to_owned(),
        instruction,
        context.cursor_overlay().style().clone(),
    )
    .with_agent_id(context.agent_id().map(str::to_owned));
    update(adapter, &control)
}

fn enabled_session(context: &CommandContext) -> Option<&str> {
    if context.cursor_overlay().is_enabled() {
        context.session_id()
    } else {
        None
    }
}

fn update(adapter: &dyn PlatformAdapter, control: &super::CursorOverlayControl) -> bool {
    if let Err(error) = adapter.update_cursor_overlay(control) {
        tracing::warn!(code = %error.code.as_str(), "agent cursor presentation was skipped");
        return false;
    }
    true
}

pub(crate) fn dispatch_mouse_event_with_cursor(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    event: MouseEvent,
    click: bool,
    lease: &crate::InteractionLease,
) -> Result<(), AdapterError> {
    let point = event.point.clone();
    crate::cursor_overlay::submit_travel(adapter, context, point.clone(), lease);
    let result = adapter.mouse_event(event, lease);
    if crate::cursor_overlay::input_was_delivered(&result) {
        crate::cursor_overlay::submit(
            adapter,
            context,
            point,
            None,
            click,
            crate::CursorPhase::Effect,
        );
    }
    result
}
