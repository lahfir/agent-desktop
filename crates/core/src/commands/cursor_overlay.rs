use crate::{AppError, CursorOverlayConfig, session::set_cursor_overlay};
use serde_json::{Value, json};

#[derive(Debug, Clone)]
pub enum CursorOverlayAction {
    Enable(CursorOverlayConfig),
    Disable,
}

pub fn execute(session_id: &str, action: CursorOverlayAction) -> Result<Value, AppError> {
    let cursor_overlay = match action {
        CursorOverlayAction::Enable(config) => config,
        CursorOverlayAction::Disable => CursorOverlayConfig::default(),
    };
    let manifest = set_cursor_overlay(session_id, cursor_overlay)?;
    Ok(json!({
        "session_id": manifest.id,
        "cursor_overlay": manifest.cursor_overlay,
    }))
}
