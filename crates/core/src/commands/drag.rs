use crate::{
    action::DragParams,
    adapter::PlatformAdapter,
    commands::helpers::{PointResolveArgs, resolve_point_from_ref_or_xy_with_context},
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
    let from = resolve_point_from_ref_or_xy_with_context(
        PointResolveArgs {
            ref_id: args.from_ref.as_deref(),
            xy: args.from_xy,
            snapshot_id: args.snapshot_id.as_deref(),
            missing_input_message: "Provide --from <ref> or --from-xy x,y",
        },
        adapter,
        context,
    )?;
    let to = resolve_point_from_ref_or_xy_with_context(
        PointResolveArgs {
            ref_id: args.to_ref.as_deref(),
            xy: args.to_xy,
            snapshot_id: args.snapshot_id.as_deref(),
            missing_input_message: "Provide --to <ref> or --to-xy x,y",
        },
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
