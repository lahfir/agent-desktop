use crate::{
    AppError, MouseButton, MouseEvent, MouseEventKind,
    adapter::PlatformAdapter,
    commands::{
        helpers::{apply_post_action_wait, validate_post_action_wait},
        point_resolve::{PointResolveArgs, require_cursor_policy},
        pointer_action::{
            PointResolveAttempt, ensure_point_deadline, focus_point_under_lease, point_deadline,
            resolve_point_under_lease, retry_leased_point_phase, wait_for_point_with_deadline,
        },
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
    let point_args = PointResolveArgs {
        ref_id: args.ref_id.as_deref(),
        xy: args.xy,
        snapshot_id: args.snapshot_id.as_deref(),
        missing_input_message: "Provide a ref (@e1) or --xy x,y",
        headed_requirement: crate::HeadedRequirement::FocusedWindowAndCursor,
    };
    let auto_wait = args.timeout_ms.is_some_and(|timeout_ms| timeout_ms > 0);
    if auto_wait {
        wait_for_point_with_deadline(point_args, deadline, adapter, context)?;
    }
    let (resolved, lease) = retry_leased_point_phase(args.timeout_ms, deadline, || {
        let lease = adapter.acquire_interaction_lease(deadline)?;
        let focused = focus_point_under_lease(point_args, &lease, adapter, context)?;
        let first = resolve_point_under_lease(
            PointResolveAttempt {
                args: point_args,
                stability: None,
                allow_scroll: !auto_wait,
            },
            deadline,
            &lease,
            adapter,
            context,
        )?;
        let mut resolved = resolve_point_under_lease(
            PointResolveAttempt {
                args: point_args,
                stability: Some(first.bounds_hash),
                allow_scroll: false,
            },
            deadline,
            &lease,
            adapter,
            context,
        )?;
        resolved.focused = focused;
        Ok((resolved, lease))
    })?;
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
    drop(lease);
    apply_post_action_wait(response, resolved.source_entry.as_ref(), adapter, context)
}

#[cfg(test)]
#[path = "hover_tests.rs"]
mod tests;
