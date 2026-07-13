use crate::{
    AdapterError, AppError, Point, actionability, adapter::PlatformAdapter,
    commands::helpers::resolve_ref_with_context, context::CommandContext,
};

#[derive(Clone, Copy)]
pub(crate) struct PointResolveArgs<'a> {
    pub ref_id: Option<&'a str>,
    pub xy: Option<(f64, f64)>,
    pub snapshot_id: Option<&'a str>,
    pub missing_input_message: &'a str,
    pub headed_requirement: crate::HeadedRequirement,
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
    crate::headed_focus::focus_entry_window(entry, adapter, context, lease)?;
    Ok(true)
}

#[cfg(test)]
#[path = "point_resolve_tests.rs"]
mod tests;
