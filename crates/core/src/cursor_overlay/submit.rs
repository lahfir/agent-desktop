use super::CursorPhase;
use crate::{CommandContext, PlatformAdapter, Point, Rect};

pub(crate) fn submit(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    target: Option<Rect>,
    click: bool,
    phase: CursorPhase,
) {
    if context.is_headed() || !context.cursor_overlay().is_enabled() {
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
    );
    if let Err(error) = adapter.update_cursor_overlay(&control) {
        tracing::warn!(code = %error.code.as_str(), "agent cursor presentation was skipped");
    }
}
