use crate::{
    AppError, MouseButton, MouseEvent, MouseEventKind,
    adapter::PlatformAdapter,
    commands::{
        helpers::{apply_post_action_wait, validate_post_action_wait},
        point_resolve::{PointResolveArgs, require_cursor_policy},
        pointer_action::{ensure_point_deadline, point_deadline, resolve_point_with_deadline},
    },
    context::CommandContext,
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
    if args.duration_ms.is_some_and(|duration| duration > 0) {
        return Err(AppError::invalid_input_with_suggestion(
            "Hover duration is unavailable because a stateless process cannot guarantee cursor ownership during a dwell",
            "Run hover without --duration, then use `wait <ms>` for an explicit post-hover pause.",
        ));
    }
    require_cursor_policy(context, "hover")?;
    validate_post_action_wait(context)?;
    let deadline = point_deadline(args.timeout_ms)?;
    let lease = adapter.acquire_interaction_lease(deadline)?;
    let resolved = resolve_point_with_deadline(
        PointResolveArgs {
            ref_id: args.ref_id.as_deref(),
            xy: args.xy,
            snapshot_id: args.snapshot_id.as_deref(),
            missing_input_message: "Provide a ref (@e1) or --xy x,y",
            headed_requirement: crate::HeadedRequirement::FocusedWindowAndCursor,
        },
        deadline,
        &lease,
        adapter,
        context,
    )?;
    ensure_point_deadline(
        deadline,
        Some(json!({
            "status": "actionable",
            "point": { "x": resolved.point.x, "y": resolved.point.y }
        })),
    )?;
    adapter.mouse_event(
        MouseEvent {
            kind: MouseEventKind::Move,
            point: resolved.point.clone(),
            button: MouseButton::Left,
            modifiers: Vec::new(),
        },
        &lease,
    )?;
    let mut response = json!({ "hovered": true, "x": resolved.point.x, "y": resolved.point.y });
    if resolved.focused {
        response["focused"] = json!(true);
    }
    apply_post_action_wait(
        response,
        resolved.source_entry.as_ref(),
        adapter,
        context,
        &lease,
    )
}

#[cfg(test)]
#[path = "hover_tests.rs"]
mod tests;
