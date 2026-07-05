use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{Value, json};

pub struct MouseWheelArgs {
    pub x: f64,
    pub y: f64,
    pub dy: i32,
    pub dx: i32,
    pub modifiers: Vec<crate::action::Modifier>,
}

pub fn execute(args: MouseWheelArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    adapter.mouse_wheel(args.x, args.y, args.dy, args.dx, &args.modifiers)?;
    Ok(json!({ "scrolled": true, "dy": args.dy, "dx": args.dx }))
}

#[cfg(test)]
#[path = "mouse_wheel_tests.rs"]
mod tests;
