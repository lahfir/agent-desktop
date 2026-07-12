use crate::{
    AppError, Modifier, MouseButton, adapter::PlatformAdapter,
    commands::point_resolve::require_cursor_policy, context::CommandContext,
};
use serde_json::Value;

pub struct MouseDownArgs {
    pub x: f64,
    pub y: f64,
    pub button: MouseButton,
    pub modifiers: Vec<Modifier>,
}

pub fn execute(
    _args: MouseDownArgs,
    _adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    require_cursor_policy(context, "mouse-down")?;
    Err(crate::commands::input_hold_policy::reject(
        "mouse-down",
        "mouse-click or drag",
    ))
}

#[cfg(test)]
#[path = "mouse_down_tests.rs"]
mod tests;
