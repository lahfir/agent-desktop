use crate::{
    action::Point,
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref_with_context,
    context::CommandContext,
    error::{AdapterError, AppError},
};
use serde_json::json;

pub(crate) struct PointResolveArgs<'a> {
    pub ref_id: Option<&'a str>,
    pub xy: Option<(f64, f64)>,
    pub snapshot_id: Option<&'a str>,
    pub missing_input_message: &'a str,
}

pub(crate) struct ResolvedPoint {
    pub point: Point,
    pub pid: Option<i32>,
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
) -> Result<ResolvedPoint, AppError> {
    if let Some(ref_id) = args.ref_id {
        let (entry, handle) = resolve_ref_with_context(ref_id, args.snapshot_id, adapter, context)?;
        let bounds = adapter
            .get_element_bounds(handle.handle())?
            .ok_or_else(|| AppError::invalid_input(format!("Element {ref_id} has no bounds")))?;
        return Ok(ResolvedPoint {
            point: Point {
                x: bounds.x + bounds.width / 2.0,
                y: bounds.y + bounds.height / 2.0,
            },
            pid: Some(entry.pid),
        });
    }
    if let Some((x, y)) = args.xy {
        return Ok(ResolvedPoint {
            point: Point { x, y },
            pid: None,
        });
    }
    Err(AppError::invalid_input(args.missing_input_message))
}

/// Ensures the app that owns a ref-resolved point is frontmost before
/// physical input is synthesized, so the events land on its window instead
/// of whatever happens to be frontmost. Headless never steals focus
/// (`--headed` opts in), and a raise failure downgrades to un-focused input
/// rather than erroring.
pub(crate) fn focus_for_physical_input(
    pid: Option<i32>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<bool, AppError> {
    let Some(pid) = pid else { return Ok(false) };
    if !context.physical_input_policy().allow_focus_steal {
        return Ok(false);
    }
    let focused = match adapter.focus_app(pid) {
        Ok(()) => true,
        Err(err) => {
            tracing::debug!("focus before physical input failed for pid {pid}: {err}");
            false
        }
    };
    context.trace_lazy("input.focus_app", || json!({ "pid": pid, "ok": focused }))?;
    Ok(focused)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_input_requires_headed_context() {
        let err = require_cursor_policy(&CommandContext::default(), "mouse-move").unwrap_err();

        assert_eq!(err.code(), "POLICY_DENIED");
    }

    #[test]
    fn headed_context_allows_physical_input() {
        require_cursor_policy(&CommandContext::default().with_headed(true), "mouse-move").unwrap();
    }
}
