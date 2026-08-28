use agent_desktop_core::{
    AppError, CursorOverlayControl, PlatformAdapter, commands::cursor_overlay,
    context::CommandContext,
};
use serde_json::Value;

use crate::cli_args::{
    cursor_overlay::CursorOverlayArgs, cursor_overlay_action::CursorOverlayAction,
};

pub(crate) fn dispatch(
    args: CursorOverlayArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let session_id = context.session_id().ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            "Cursor overlay settings require an active session",
            "Run `session start`, then pass its id with --session or AGENT_DESKTOP_SESSION.",
        )
    })?;
    let (action, control) = match args.action {
        CursorOverlayAction::Enable(args) => {
            let config = args.to_core()?;
            let control =
                CursorOverlayControl::enable(session_id.to_owned(), config.style().clone());
            (
                cursor_overlay::CursorOverlayAction::Enable(config),
                Some(control),
            )
        }
        CursorOverlayAction::Disable => (
            cursor_overlay::CursorOverlayAction::Disable,
            Some(CursorOverlayControl::disable(session_id.to_owned())),
        ),
    };
    let value = cursor_overlay::execute(session_id, action)?;
    if let Some(control) = control
        && let Err(error) = adapter.update_cursor_overlay(&control)
    {
        tracing::warn!(code = %error.code.as_str(), "cursor overlay lifecycle update was skipped");
    }
    Ok(value)
}
