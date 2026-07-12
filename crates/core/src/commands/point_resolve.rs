use crate::{
    AdapterError, AppError, Point, actionability, adapter::PlatformAdapter,
    commands::helpers::resolve_ref_with_context, context::CommandContext,
};
use serde_json::json;

#[derive(Clone, Copy)]
pub(crate) struct PointResolveArgs<'a> {
    pub ref_id: Option<&'a str>,
    pub xy: Option<(f64, f64)>,
    pub snapshot_id: Option<&'a str>,
    pub missing_input_message: &'a str,
    pub focus_before_resolve: bool,
}

pub(crate) struct ResolvedPoint {
    pub point: Point,
    pub focused: bool,
    pub source_entry: Option<crate::RefEntry>,
}

pub(crate) fn require_cursor_policy(
    context: &CommandContext,
    command: &str,
) -> Result<(), AppError> {
    let policy = context.physical_input_policy();
    if policy.allow_cursor_move {
        return Ok(());
    }
    Err(AdapterError::policy_denied_for_policy(
        format!("{command} moves the cursor and is disabled in headless mode"),
        policy,
    )
    .into())
}

pub(crate) fn resolve_point_from_ref_or_xy_with_context(
    args: PointResolveArgs<'_>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    deadline: crate::Deadline,
    _lease: &crate::InteractionLease,
) -> Result<ResolvedPoint, AppError> {
    if let Some(ref_id) = args.ref_id {
        let (entry, handle) = resolve_ref_with_context(ref_id, args.snapshot_id, adapter, context)?;
        let bounds = adapter
            .get_element_bounds(&handle, deadline)?
            .ok_or_else(|| AppError::invalid_input(format!("Element {ref_id} has no bounds")))?;
        let point = Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height / 2.0,
        };
        actionability::require_receives_events(&handle, point.clone(), adapter, deadline)?;
        return Ok(ResolvedPoint {
            point,
            focused: false,
            source_entry: Some(entry),
        });
    }
    if let Some((x, y)) = args.xy {
        return Ok(ResolvedPoint {
            point: Point { x, y },
            focused: false,
            source_entry: None,
        });
    }
    Err(AppError::invalid_input(args.missing_input_message))
}

pub(crate) fn focus_for_physical_input(
    entry: Option<&crate::RefEntry>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    lease: &crate::InteractionLease,
) -> Result<bool, AppError> {
    let Some(entry) = entry else { return Ok(false) };
    if !context.physical_input_policy().allow_focus_steal {
        return Ok(false);
    }
    let process_instance = entry
        .process
        .process_instance
        .clone()
        .filter(|instance| !instance.is_empty());
    let Some(process_instance) = process_instance else {
        return Ok(false);
    };
    let window_id = entry
        .source
        .source_window_id
        .clone()
        .filter(|id| !id.is_empty());
    let Some(window_id) = window_id else {
        return Ok(false);
    };
    let expected = crate::WindowInfo {
        id: window_id,
        title: entry.source.source_window_title.clone().unwrap_or_default(),
        app: entry.source.source_app.clone().unwrap_or_default(),
        pid: entry.process.pid,
        process_instance: Some(process_instance.clone()),
        bounds: None,
        state: crate::WindowState {
            is_focused: false,
            ..Default::default()
        },
    };
    let live = match adapter.resolve_window_strict(&expected, lease.deadline()) {
        Ok(live) => live,
        Err(error) => return focus_failure(error),
    };
    if live.id != expected.id
        || live.pid != expected.pid
        || live.process_instance.as_deref() != Some(process_instance.as_str())
    {
        return Err(AdapterError::stale_ref(
            "target source window belongs to a different process instance",
        )
        .into());
    }
    if let Err(error) = adapter.focus_window(&live, lease) {
        return focus_failure(error);
    }
    let focused = true;
    context.trace_lazy(
        "input.focus_window",
        || json!({ "pid": live.pid, "window_id": live.id, "ok": focused }),
    )?;
    Ok(focused)
}

fn focus_failure(error: AdapterError) -> Result<bool, AppError> {
    if matches!(
        error.code,
        crate::ErrorCode::PermDenied
            | crate::ErrorCode::PolicyDenied
            | crate::ErrorCode::StaleRef
            | crate::ErrorCode::AmbiguousTarget
            | crate::ErrorCode::InvalidArgs
            | crate::ErrorCode::Internal
    ) {
        return Err(error.into());
    }
    Ok(false)
}

#[cfg(test)]
#[path = "point_resolve_tests.rs"]
mod tests;
