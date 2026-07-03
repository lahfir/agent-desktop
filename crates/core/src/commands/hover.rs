use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind},
    adapter::PlatformAdapter,
    commands::{
        helpers::resolve_point_with_wait,
        point_resolve::{focus_for_physical_input, require_cursor_policy},
    },
    context::CommandContext,
    error::AppError,
};
use serde_json::{Value, json};

pub struct HoverArgs {
    pub ref_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub xy: Option<(f64, f64)>,
    pub duration_ms: Option<u64>,
    pub timeout_ms: Option<u64>,
}

pub fn execute(
    args: HoverArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    require_cursor_policy(context, "hover")?;
    let resolved = resolve_point_with_wait(
        args.ref_id.as_deref(),
        args.xy,
        args.snapshot_id.as_deref(),
        "Provide a ref (@e1) or --xy x,y",
        args.timeout_ms,
        adapter,
        context,
    )?;
    let focused = focus_for_physical_input(resolved.pid, adapter, context)?;
    adapter.mouse_event(MouseEvent {
        kind: MouseEventKind::Move,
        point: resolved.point.clone(),
        button: MouseButton::Left,
        modifiers: Vec::new(),
    })?;
    if let Some(ms) = args.duration_ms {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    let mut response = json!({ "hovered": true, "x": resolved.point.x, "y": resolved.point.y });
    if focused {
        response["focused"] = json!(true);
    }
    Ok(response)
}

#[cfg(test)]
#[path = "hover_tests.rs"]
mod tests;
