use crate::{
    action::{Modifier, MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    commands::point_resolve::require_cursor_policy,
    context::CommandContext,
    error::AppError,
};
use serde_json::{Value, json};

pub struct MouseUpArgs {
    pub x: f64,
    pub y: f64,
    pub button: MouseButton,
    pub modifiers: Vec<Modifier>,
}

pub fn execute(
    args: MouseUpArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    require_cursor_policy(context, "mouse-up")?;
    adapter.mouse_event(MouseEvent {
        kind: MouseEventKind::Up,
        point: Point {
            x: args.x,
            y: args.y,
        },
        button: args.button,
        modifiers: args.modifiers,
    })?;
    Ok(json!({ "released": true, "x": args.x, "y": args.y }))
}

#[cfg(test)]
#[path = "mouse_up_tests.rs"]
mod tests;
