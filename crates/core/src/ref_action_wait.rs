use crate::{
    action_request::ActionRequest,
    action_result::ActionResult,
    actionability,
    adapter::PlatformAdapter,
    context::CommandContext,
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
    resolved_element::ResolvedElement,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

fn ensure_process_responsive(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
) -> Result<(), AdapterError> {
    let state = match adapter.process_state(entry.pid) {
        Ok(state) => state,
        Err(err) if err.code == ErrorCode::PlatformNotSupported => return Ok(()),
        Err(err) => return Err(err),
    };
    if state == crate::process_state::ProcessState::Unresponsive {
        let app = entry.source_app.as_deref().unwrap_or("target application");
        return Err(AdapterError::app_unresponsive(app));
    }
    Ok(())
}

fn trace_resolve_error(context: &CommandContext, ref_id: &str, err: &AdapterError) {
    let _ = context.trace_lazy("ref.resolve.error", || {
        json!({
            "ref": ref_id,
            "code": err.code.as_str(),
            "message": err.message.clone(),
            "details": err.details.clone()
        })
    });
}

pub(crate) const POLL_INTERVAL: Duration = Duration::from_millis(100);
pub(crate) const RESOLVE_ATTEMPT: Duration = Duration::from_millis(750);

pub(crate) fn execute_with_auto_wait(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    ref_id: &str,
    context: &CommandContext,
    request: ActionRequest,
    dispatch: impl FnOnce(
        crate::ref_action::ResolvedRefAction<'_>,
        ActionRequest,
    ) -> Result<ActionResult, crate::error::AppError>,
) -> Result<ActionResult, AdapterError> {
    ensure_process_responsive(adapter, entry)?;
    let Some(budget_ms) = request.timeout_ms else {
        return execute_single_shot(adapter, entry, ref_id, context, request, dispatch);
    };
    execute_poll_loop(
        adapter,
        entry,
        ref_id,
        context,
        request,
        Duration::from_millis(budget_ms),
        dispatch,
    )
}

fn execute_single_shot(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    ref_id: &str,
    context: &CommandContext,
    request: ActionRequest,
    dispatch: impl FnOnce(
        crate::ref_action::ResolvedRefAction<'_>,
        ActionRequest,
    ) -> Result<ActionResult, crate::error::AppError>,
) -> Result<ActionResult, AdapterError> {
    let handle = adapter
        .resolve_element_strict(entry)
        .inspect_err(|err| trace_resolve_error(context, ref_id, err))?;
    let handle = ResolvedElement::new(adapter, handle);
    maybe_scroll_into_view(adapter, entry, handle.handle(), &request);
    dispatch(
        crate::ref_action::ResolvedRefAction {
            adapter,
            entry,
            handle: handle.handle(),
            ref_id,
            context,
        },
        request,
    )
    .map_err(crate::ref_action::into_adapter_error)
}

fn execute_poll_loop(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    ref_id: &str,
    context: &CommandContext,
    request: ActionRequest,
    budget: Duration,
    dispatch: impl FnOnce(
        crate::ref_action::ResolvedRefAction<'_>,
        ActionRequest,
    ) -> Result<ActionResult, crate::error::AppError>,
) -> Result<ActionResult, AdapterError> {
    let deadline = Instant::now() + budget;
    let mut last_report: Option<Value> = None;
    let mut saw_ambiguity = false;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(actionability_timeout(last_report));
        }
        let attempt = remaining.min(RESOLVE_ATTEMPT);
        match adapter.resolve_element_strict_with_timeout(entry, attempt) {
            Ok(handle) => {
                let resolved = ResolvedElement::new(adapter, handle);
                maybe_scroll_into_view(adapter, entry, resolved.handle(), &request);
                match actionability::check_live(entry, resolved.handle(), adapter, &request) {
                    Ok(_report) => {
                        let mut result = dispatch(
                            crate::ref_action::ResolvedRefAction {
                                adapter,
                                entry,
                                handle: resolved.handle(),
                                ref_id,
                                context,
                            },
                            request.clone(),
                        )
                        .map_err(crate::ref_action::into_adapter_error)?;
                        if saw_ambiguity {
                            result = result.with_details(json!({ "transient_ambiguity": true }));
                        }
                        return Ok(result);
                    }
                    Err(err) => {
                        let code = err.code.clone();
                        if is_permanent_error(&code) {
                            return Err(err);
                        }
                        last_report = err.details.clone();
                        sleep_poll_interval(deadline);
                    }
                }
            }
            Err(err) => {
                let code = err.code.clone();
                if is_permanent_error(&code) {
                    trace_resolve_error(context, ref_id, &err);
                    return Err(err);
                }
                if code == ErrorCode::AmbiguousTarget {
                    saw_ambiguity = true;
                } else if !is_retryable_resolve_error(&code) {
                    trace_resolve_error(context, ref_id, &err);
                    return Err(err);
                }
                sleep_poll_interval(deadline);
            }
        }
    }
}

fn sleep_poll_interval(deadline: Instant) {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return;
    }
    std::thread::sleep(POLL_INTERVAL.min(remaining));
}

fn is_permanent_error(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::PermDenied
            | ErrorCode::AppNotFound
            | ErrorCode::ActionNotSupported
            | ErrorCode::InvalidArgs
            | ErrorCode::PolicyDenied
            | ErrorCode::AppUnresponsive
    )
}

fn is_retryable_resolve_error(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::StaleRef | ErrorCode::AmbiguousTarget | ErrorCode::Timeout
    )
}

fn maybe_scroll_into_view(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    handle: &crate::adapter::NativeHandle,
    request: &ActionRequest,
) {
    if !request.action.requires_scroll_into_view() {
        return;
    }
    let needs_scroll = crate::state::has_state(&entry.states, crate::state::OFFSCREEN)
        || entry
            .bounds
            .is_none_or(|bounds| bounds.width <= 0.0 || bounds.height <= 0.0);
    if !needs_scroll {
        return;
    }
    let _ = adapter.scroll_into_view(handle);
}

fn actionability_timeout(last_report: Option<Value>) -> AdapterError {
    let mut details = json!({ "kind": "actionability_timeout" });
    if let Some(report) = last_report {
        if let Some(obj) = details.as_object_mut() {
            obj.insert("report".into(), report);
        }
    }
    AdapterError::timeout("Target did not become actionable within the wait budget")
        .with_details(details)
}

#[cfg(test)]
#[path = "ref_action_wait_tests.rs"]
mod tests;
