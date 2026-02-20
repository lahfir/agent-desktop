use crate::{
    action::{MouseButton, MouseEvent, MouseEventKind, Point},
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref,
    error::AppError,
};
use serde_json::{json, Value};

pub struct HoverArgs {
    pub ref_id: Option<String>,
    pub xy: Option<(f64, f64)>,
    pub duration_ms: Option<u64>,
}

pub fn execute(args: HoverArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let point = resolve_hover_point(&args, adapter)?;
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
) -> Result<Point, AppError> {
    if let Some(ref_id) = &args.ref_id {
        let (_entry, handle) = resolve_ref(ref_id, adapter)?;
        let bounds = adapter
            .get_element_bounds(&handle)?
            .ok_or_else(|| AppError::invalid_input(format!("Element {ref_id} has no bounds")))?;
        Ok(Point { x: bounds.x + bounds.width / 2.0, y: bounds.y + bounds.height / 2.0 })
    } else if let Some((x, y)) = args.xy {
        Ok(Point { x, y })
    } else {
        Err(AppError::invalid_input("Provide a ref (@e1) or --xy x,y"))
    }
}
