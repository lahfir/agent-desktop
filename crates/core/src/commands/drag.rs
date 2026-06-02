use crate::{
    action::{DragParams, Point},
    adapter::PlatformAdapter,
    commands::helpers::resolve_point_from_ref_or_xy_with_context,
    context::CommandContext,
    error::AppError,
};
use serde_json::{Value, json};

pub struct DragArgs {
    pub from_ref: Option<String>,
    pub from_xy: Option<(f64, f64)>,
    pub to_ref: Option<String>,
    pub to_xy: Option<(f64, f64)>,
    pub snapshot_id: Option<String>,
    pub duration_ms: Option<u64>,
}

pub fn execute(
    args: DragArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let from = resolve_point(
        &args.from_ref,
        args.from_xy,
        "from",
        args.snapshot_id.as_deref(),
        adapter,
        context,
    )?;
    let to = resolve_point(
        &args.to_ref,
        args.to_xy,
        "to",
        args.snapshot_id.as_deref(),
        adapter,
        context,
    )?;
    let params = DragParams {
        from: from.clone(),
        to: to.clone(),
        duration_ms: args.duration_ms,
    };
    adapter.drag(params)?;
    Ok(json!({
        "dragged": true,
        "from": { "x": from.x, "y": from.y },
        "to": { "x": to.x, "y": to.y }
    }))
}

fn resolve_point(
    ref_id: &Option<String>,
    xy: Option<(f64, f64)>,
    label: &str,
    snapshot_id: Option<&str>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Point, AppError> {
    resolve_point_from_ref_or_xy_with_context(
        ref_id.as_deref(),
        xy,
        snapshot_id,
        adapter,
        format!("Provide --{label} <ref> or --{label}-xy x,y"),
        context,
    )
}
