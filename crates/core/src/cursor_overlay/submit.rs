use crate::{CommandContext, PlatformAdapter, Point};

pub(crate) fn submit(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    destination: Point,
    click: bool,
) {
    if context.is_headed() || !context.cursor_overlay().is_enabled() {
        return;
    }
    let instruction =
        match super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), click) {
            Ok(instruction) => instruction,
            Err(error) => {
                tracing::warn!(code = %error.code.as_str(), "agent cursor instruction was skipped");
                return;
            }
        };
    if let Err(error) = adapter.present_cursor_overlay(&instruction) {
        tracing::warn!(code = %error.code.as_str(), "agent cursor presentation was skipped");
    }
}
