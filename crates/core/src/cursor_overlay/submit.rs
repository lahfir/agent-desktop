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
    let Some(session_id) = context.session_id() else {
        return;
    };
    let instruction =
        match super::CursorOverlayInstruction::new(destination, context.cursor_overlay(), click) {
            Ok(instruction) => instruction,
            Err(error) => {
                tracing::warn!(code = %error.code.as_str(), "agent cursor instruction was skipped");
                return;
            }
        };
    let control = super::CursorOverlayControl::present(session_id.to_owned(), instruction);
    if let Err(error) = adapter.update_cursor_overlay(&control) {
        tracing::warn!(code = %error.code.as_str(), "agent cursor presentation was skipped");
    }
}
