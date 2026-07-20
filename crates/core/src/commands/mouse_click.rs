use crate::{
    AppError, Modifier, MouseButton, MouseEvent, MouseEventKind, Point, adapter::PlatformAdapter,
    commands::point_resolve::require_cursor_policy, context::CommandContext,
};
use serde_json::{Value, json};

pub struct MouseClickArgs {
    pub x: f64,
    pub y: f64,
    pub button: MouseButton,
    pub count: u32,
    pub modifiers: Vec<Modifier>,
}

pub fn execute(
    args: MouseClickArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    require_cursor_policy(context, "mouse-click")?;
    crate::validate_mouse_click_count(args.count)?;
    let lease = crate::commands::helpers::acquire_interaction_lease(adapter)?;
    let point = Point {
        x: args.x,
        y: args.y,
    };
    point.validate()?;
    adapter.mouse_event(
        MouseEvent {
            kind: MouseEventKind::Click { count: args.count },
            point,
            button: args.button,
            modifiers: args.modifiers,
        },
        &lease,
    )?;
    Ok(json!({ "clicked": true, "x": args.x, "y": args.y, "count": args.count }))
}

#[cfg(test)]
#[path = "mouse_click_tests.rs"]
mod tests;
