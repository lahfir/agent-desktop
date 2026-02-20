use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    error::AppError,
};
use serde_json::{json, Value};

pub struct MouseMoveArgs {
    pub x: f64,
    pub y: f64,
}

pub fn execute(args: MouseMoveArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
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
