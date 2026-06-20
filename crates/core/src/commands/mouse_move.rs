use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    commands::point_resolve::require_cursor_policy,
    context::CommandContext,
    error::AppError,
};
use serde_json::{Value, json};

pub struct MouseMoveArgs {
    pub x: f64,
    pub y: f64,
}

pub fn execute(
    args: MouseMoveArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    require_cursor_policy(context, "mouse-move")?;
    adapter.mouse_event(MouseEvent {
        kind: MouseEventKind::Move,
        point: Point {
            x: args.x,
            y: args.y,
        },
        button: MouseButton::Left,
    })?;
    Ok(json!({ "moved": true, "x": args.x, "y": args.y }))
}
