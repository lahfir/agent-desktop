use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    error::AppError,
};
use serde_json::{json, Value};

pub struct MouseClickArgs {
    pub x: f64,
    pub y: f64,
    pub button: MouseButton,
    pub count: u32,
}

pub fn execute(args: MouseClickArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    adapter.mouse_event(MouseEvent {
        kind: MouseEventKind::Click { count: args.count },
        point: Point { x: args.x, y: args.y },
        button: args.button,
    })?;
    Ok(json!({ "clicked": true, "x": args.x, "y": args.y, "count": args.count }))
}
