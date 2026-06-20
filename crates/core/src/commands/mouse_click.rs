use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    commands::point_resolve::require_cursor_policy,
    context::CommandContext,
    error::AppError,
};
use serde_json::{Value, json};

pub struct MouseClickArgs {
    pub x: f64,
    pub y: f64,
    pub button: MouseButton,
    pub count: u32,
}

pub fn execute(
    args: MouseClickArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    require_cursor_policy(context, "mouse-click")?;
    adapter.mouse_event(MouseEvent {
        kind: MouseEventKind::Click { count: args.count },
        point: Point {
            x: args.x,
            y: args.y,
        },
        button: args.button,
    })?;
    Ok(json!({ "clicked": true, "x": args.x, "y": args.y, "count": args.count }))
}
