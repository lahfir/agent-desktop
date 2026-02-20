use crate::{
    action::{DragParams, Point},
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref,
    error::AppError,
};
use serde_json::{json, Value};

pub struct DragArgs {
    pub from_ref: Option<String>,
    pub from_xy: Option<(f64, f64)>,
    pub to_ref: Option<String>,
    pub to_xy: Option<(f64, f64)>,
    pub duration_ms: Option<u64>,
}

pub fn execute(args: DragArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let from = resolve_point(&args.from_ref, args.from_xy, "from", adapter)?;
    let to = resolve_point(&args.to_ref, args.to_xy, "to", adapter)?;
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
    adapter: &dyn PlatformAdapter,
) -> Result<Point, AppError> {
    if let Some(ref_id) = ref_id {
        let (_entry, handle) = resolve_ref(ref_id, adapter)?;
        let bounds = adapter
            .get_element_bounds(&handle)?
            .ok_or_else(|| AppError::invalid_input(format!("Element {ref_id} has no bounds")))?;
        Ok(Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        })
    } else if let Some((x, y)) = xy {
        Ok(Point { x, y })
    } else {
        Err(AppError::invalid_input(format!(
            "Provide --{label} <ref> or --{label}-xy x,y"
        )))
    }
}
