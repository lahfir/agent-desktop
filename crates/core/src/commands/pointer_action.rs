use crate::{
    AppError,
    adapter::PlatformAdapter,
    commands::helpers::{load_ref_entry, resolve_handle_within_deadline},
    context::CommandContext,
    ref_resolve_deadline::POLL_INTERVAL,
    refs::RefEntry,
};
use serde_json::{Value, json};

fn resolve_point_from_entry(
    target: (&str, &RefEntry),
    stability: Option<Option<u64>>,
    deadline: crate::Deadline,
    lease: &crate::InteractionLease,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<crate::commands::point_resolve::ResolvedPoint, AppError> {
    let (ref_id, entry) = target;
    let mut handle =
        resolve_handle_within_deadline(adapter, entry, deadline).inspect_err(|err| {
            let _ = context.trace_lazy("ref.resolve.error", || {
                json!({
                    "ref": ref_id,
                    "code": err.code.as_str(),
                    "message": err.message.clone(),
                    "details": err.details.clone()
                })
            });
        })?;
    context.trace_lazy("ref.resolve.ok", || json!({ "ref": ref_id }))?;
    let (mut bounds, mut states) = pointer_live_observation(adapter, &handle, deadline)?;
    if !pointer_is_visible(bounds, &states) {
        if deadline.is_expired() {
            return Err(crate::AdapterError::timeout(
                "Pointer target did not become visible within the wait budget",
            )
            .into());
        }
        adapter.scroll_into_view(&handle, lease)?;
        handle = resolve_handle_within_deadline(adapter, entry, deadline)?;
        (bounds, states) = pointer_live_observation(adapter, &handle, deadline)?;
        if !pointer_is_visible(bounds, &states) {
            return Err(crate::AdapterError::new(
                crate::ErrorCode::ActionFailed,
                "Pointer target is not visible in the live accessibility tree",
            )
            .with_details(json!({ "check": "visible" }))
            .into());
        }
    }
    let Some(bounds) = bounds else {
        return Err(crate::AdapterError::new(
            crate::ErrorCode::ActionFailed,
            format!("Element {ref_id} has no live bounds"),
        )
        .with_details(json!({ "check": "visible" }))
        .into());
    };
    let observed_bounds_hash = bounds.bounds_hash().ok_or_else(|| {
        crate::AdapterError::new(
            crate::ErrorCode::ActionFailed,
            "Pointer target exposes invalid live bounds",
        )
    })?;
    if stability.is_some_and(|expected| expected != Some(observed_bounds_hash)) {
        return Err(crate::AdapterError::new(
            crate::ErrorCode::ActionFailed,
            "Pointer target bounds are not stable yet",
        )
        .with_details(json!({
            "check": "stable",
            "observed_bounds_hash": observed_bounds_hash
        }))
        .into());
    }
    let point = crate::Point {
        x: bounds.x + bounds.width / 2.0,
        y: bounds.y + bounds.height / 2.0,
    };
    crate::actionability::require_receives_events(&handle, point.clone(), adapter, deadline)?;
    if deadline.is_expired() {
        return Err(crate::AdapterError::timeout(
            "Pointer target did not become actionable within the wait budget",
        )
        .into());
    }
    Ok(crate::commands::point_resolve::ResolvedPoint {
        point,
        focused: false,
        source_entry: Some(entry.clone()),
    })
}

fn pointer_live_observation(
    adapter: &dyn PlatformAdapter,
    handle: &crate::adapter::NativeHandle,
    deadline: crate::Deadline,
) -> Result<(Option<crate::Rect>, Vec<String>), AppError> {
    let bounds = adapter.get_element_bounds(handle, deadline)?;
    let states = crate::adapter::optional_live_read(adapter.get_live_state(handle, deadline))?
        .map(|state| state.states)
        .unwrap_or_default();
    Ok((bounds, states))
}

fn pointer_is_visible(bounds: Option<crate::Rect>, states: &[String]) -> bool {
    !crate::state::has_state(states, crate::state::HIDDEN)
        && !crate::state::has_state(states, crate::state::OFFSCREEN)
        && bounds.is_some_and(|bounds| {
            bounds.validate().is_ok() && bounds.width > 0.0 && bounds.height > 0.0
        })
}

pub(crate) fn point_deadline(timeout_ms: Option<u64>) -> Result<crate::Deadline, AppError> {
    timeout_ms
        .map_or_else(crate::Deadline::standard, crate::Deadline::after)
        .map_err(AppError::Adapter)
}

pub(crate) fn resolve_point_with_deadline<'a>(
    args: crate::commands::point_resolve::PointResolveArgs<'a>,
    deadline: crate::Deadline,
    lease: &crate::InteractionLease,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<crate::commands::point_resolve::ResolvedPoint, AppError> {
    use crate::commands::point_resolve::resolve_point_from_ref_or_xy_with_context;

    let Some(ref_id) = args.ref_id else {
        return resolve_point_from_ref_or_xy_with_context(args, adapter, context, deadline, lease);
    };
    let entry = load_ref_entry(ref_id, args.snapshot_id, context)?;
    let focused = if args.headed_requirement.requires_focus() {
        crate::commands::point_resolve::focus_for_physical_input(
            Some(&entry),
            adapter,
            context,
            lease,
        )?
    } else {
        false
    };
    let mut stability = Some(None);
    let mut last_report = None;
    loop {
        if deadline.is_expired() {
            return Err(point_actionability_timeout(last_report));
        }
        match resolve_point_from_entry(
            (ref_id, &entry),
            stability,
            deadline,
            lease,
            adapter,
            context,
        ) {
            Ok(mut point) => {
                point.focused = focused;
                return Ok(point);
            }
            Err(err) => {
                let remaining = deadline.remaining();
                if !is_retryable_point_error(&err) {
                    return Err(err);
                }
                last_report = Some(json!({
                    "code": err.code(),
                    "message": err.to_string(),
                    "details": match &err {
                        AppError::Adapter(adapter_error) => adapter_error.details.clone(),
                        _ => None,
                    }
                }));
                if remaining.is_zero() {
                    return Err(point_actionability_timeout(last_report));
                }
                let observed = point_observed_bounds_hash(&err);
                if let Some(observed) = observed {
                    stability = Some(Some(observed));
                }
                let interval = if observed.is_some() {
                    std::time::Duration::from_millis(16)
                } else {
                    POLL_INTERVAL
                };
                std::thread::sleep(interval.min(remaining));
            }
        }
    }
}

fn point_actionability_timeout(last_report: Option<Value>) -> AppError {
    let mut details = json!({ "kind": "actionability_timeout" });
    if let Some(last_report) = last_report
        && let Some(object) = details.as_object_mut()
    {
        object.insert("last_report".into(), last_report);
    }
    crate::AdapterError::timeout("Pointer target did not become actionable within the wait budget")
        .with_details(details)
        .into()
}

pub(crate) fn ensure_point_deadline(
    deadline: crate::Deadline,
    last_report: Option<Value>,
) -> Result<(), AppError> {
    if deadline.is_expired() {
        return Err(point_actionability_timeout(last_report));
    }
    Ok(())
}

fn point_observed_bounds_hash(err: &AppError) -> Option<u64> {
    let AppError::Adapter(err) = err else {
        return None;
    };
    err.details
        .as_ref()
        .and_then(|details| details.get("observed_bounds_hash"))
        .and_then(Value::as_u64)
}

fn is_retryable_point_error(err: &AppError) -> bool {
    match err.code() {
        "STALE_REF" | "AMBIGUOUS_TARGET" | "TIMEOUT" | "APP_UNRESPONSIVE" => {
            error_is_explicitly_retryable(err)
        }
        "ACTION_FAILED" => ["visible", "stable", "receives_events"]
            .into_iter()
            .any(|check| point_failed_check(err, check)),
        _ => false,
    }
}

fn error_is_explicitly_retryable(err: &AppError) -> bool {
    let AppError::Adapter(error) = err else {
        return false;
    };
    error.is_explicitly_retryable()
}

fn point_failed_check(err: &AppError, expected: &str) -> bool {
    let AppError::Adapter(error) = err else {
        return false;
    };
    let Some(details) = error.details.as_ref() else {
        return false;
    };
    if details.get("check").and_then(Value::as_str) == Some(expected) {
        return true;
    }
    details
        .get("checks")
        .and_then(Value::as_array)
        .is_some_and(|checks| {
            checks.iter().any(|check| {
                check.get("check").and_then(Value::as_str) == Some(expected)
                    && check.get("status").and_then(Value::as_str) != Some("pass")
            })
        })
}

#[cfg(test)]
#[path = "pointer_action_tests.rs"]
mod tests;
