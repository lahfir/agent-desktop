use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    error::AppError,
};
use serde_json::{json, Value};

pub struct MouseDownArgs {
    pub x: f64,
    pub y: f64,
    pub button: MouseButton,
}

pub fn execute(args: MouseDownArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    adapter.mouse_event(MouseEvent {
        kind: MouseEventKind::Down,
        point: Point {
            x: args.x,
            y: args.y,
        },
        button: args.button,
    })?;
    Ok(json!({ "pressed": true, "x": args.x, "y": args.y }))
}
