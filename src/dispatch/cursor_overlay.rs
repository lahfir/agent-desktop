use agent_desktop_core::{AppError, commands::cursor_overlay, context::CommandContext};
use serde_json::Value;

use crate::cli_args::{
    cursor_overlay::CursorOverlayArgs, cursor_overlay_action::CursorOverlayAction,
};

pub(crate) fn dispatch(
    args: CursorOverlayArgs,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let session_id = context.session_id().ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            "Cursor overlay settings require an active session",
            "Run `session start`, then pass its id with --session or AGENT_DESKTOP_SESSION.",
        )
    })?;
    let action = match args.action {
        CursorOverlayAction::Enable(args) => {
            cursor_overlay::CursorOverlayAction::Enable(args.to_core()?)
        }
        CursorOverlayAction::Disable => cursor_overlay::CursorOverlayAction::Disable,
    };
    cursor_overlay::execute(session_id, action)
}
