use crate::{
    AppError, MouseButton, MouseEvent, MouseEventKind, Point, adapter::PlatformAdapter,
    commands::point_resolve::require_cursor_policy, context::CommandContext,
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
    let lease = crate::commands::helpers::acquire_interaction_lease(adapter)?;
    let point = Point {
        x: args.x,
        y: args.y,
    };
    point.validate()?;
    crate::cursor_overlay::submit_travel(adapter, context, point.clone(), &lease);
    let result = adapter.mouse_event(
        MouseEvent {
            kind: MouseEventKind::Move,
            point: point.clone(),
            button: MouseButton::Left,
            modifiers: Vec::new(),
        },
        &lease,
    );
    if crate::cursor_overlay::input_was_delivered(&result) {
        crate::cursor_overlay::submit(
            adapter,
            context,
            point,
            None,
            false,
            crate::CursorPhase::Effect,
        );
    }
    result?;
    Ok(json!({ "moved": true, "x": args.x, "y": args.y }))
}
