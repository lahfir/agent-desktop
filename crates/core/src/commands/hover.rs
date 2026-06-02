use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    commands::helpers::{PointResolveArgs, resolve_point_from_ref_or_xy_with_context},
    context::CommandContext,
    error::AppError,
};
use serde_json::{Value, json};

pub struct HoverArgs {
    pub ref_id: Option<String>,
    pub snapshot_id: Option<String>,
    pub xy: Option<(f64, f64)>,
    pub duration_ms: Option<u64>,
}

pub fn execute(
    args: HoverArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let point = resolve_hover_point(&args, adapter, context)?;
    adapter.mouse_event(MouseEvent {
        kind: MouseEventKind::Move,
        point: point.clone(),
        button: MouseButton::Left,
    })?;
    if let Some(ms) = args.duration_ms {
        std::thread::sleep(std::time::Duration::from_millis(ms));
    }
    Ok(json!({ "hovered": true, "x": point.x, "y": point.y }))
}

fn resolve_hover_point(
    args: &HoverArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Point, AppError> {
    resolve_point_from_ref_or_xy_with_context(
        PointResolveArgs {
            ref_id: args.ref_id.as_deref(),
            xy: args.xy,
            snapshot_id: args.snapshot_id.as_deref(),
            missing_input_message: "Provide a ref (@e1) or --xy x,y",
        },
        adapter,
        context,
    )
}
