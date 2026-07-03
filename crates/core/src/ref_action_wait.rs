use crate::{
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::PlatformAdapter,
    context::CommandContext,
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
    resolved_element::ResolvedElement,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// Best-effort enrichment of a terminal (caller-visible) error with process
/// liveness context. Runs exactly once, on the error `execute_with_auto_wait`
/// is about to return — never on an internal auto-wait retry tick — and
/// never converts a success into a failure: it only ever transforms an
/// already-failed `Result`. Only `STALE_REF`/`APP_NOT_FOUND` are enriched
/// (the codes that plausibly indicate a dead or hung target); a probe error
/// (including `PLATFORM_NOT_SUPPORTED` on adapters without this capability)
/// leaves the original error untouched.
fn enrich_with_process_state(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    err: AdapterError,
) -> AdapterError {
    if !matches!(err.code, ErrorCode::StaleRef | ErrorCode::AppNotFound) {
        return err;
    }
    let Ok(state) = adapter.process_state(entry.pid) else {
        return err;
    };
    if state == crate::process_state::ProcessState::Unresponsive {
        let app = entry.source_app.as_deref().unwrap_or("target application");
        return AdapterError::app_unresponsive(app);
    }
    attach_process_state_detail(err, state)
}

fn attach_process_state_detail(
    err: AdapterError,
    state: crate::process_state::ProcessState,
) -> AdapterError {
    let mut details = err.details.clone().unwrap_or_else(|| json!({}));
    match details.as_object_mut() {
        Some(obj) => {
            obj.insert("process_state".into(), json!(state.label()));
        }
        None => details = json!({ "process_state": state.label() }),
    }
    err.with_details(details)
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
const MAX_BUDGET_MS: u64 = 24 * 60 * 60 * 1000;

pub(crate) fn budget_from_ms(ms: u64) -> Duration {
    Duration::from_millis(ms.min(MAX_BUDGET_MS))
}

/// Outcome of one capped resolve attempt inside a deadline-bounded poll loop.
pub(crate) enum ResolveAttemptOutcome {
    Resolved(crate::adapter::NativeHandle),
    Failed(AdapterError),
    DeadlinePassed,
}

/// Single owner of the "poll resolve, capped per attempt, until deadline"
/// math shared by the ref-action auto-wait loop and `wait --element`. Caps
/// each individual [`ObservationOps::resolve_element_strict_with_timeout`]
/// call to [`RESOLVE_ATTEMPT`] (never exceeding what's left before
/// `deadline`), so a slow resolve cannot consume the whole wait budget on a
/// single attempt. Callers own everything after the resolve: what to do with
/// a resolved handle, which failure codes are retryable for them (e.g.
/// `wait --element` additionally retries `ELEMENT_NOT_FOUND`), and any
/// side-effects like refreshing a `LatestRefCache`.
pub(crate) fn resolve_within_deadline(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: Instant,
) -> ResolveAttemptOutcome {
    let remaining = deadline.saturating_duration_since(Instant::now());
    if remaining.is_zero() {
        return ResolveAttemptOutcome::DeadlinePassed;
    }
    let attempt = remaining.min(RESOLVE_ATTEMPT);
    match adapter.resolve_element_strict_with_timeout(entry, attempt) {
        Ok(handle) => ResolveAttemptOutcome::Resolved(handle),
        Err(err) => ResolveAttemptOutcome::Failed(err),
    }
}

pub(crate) fn execute_with_auto_wait(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    ref_id: &str,
    context: &CommandContext,
    request: ActionRequest,
    dispatch: impl Fn(
        crate::ref_action::ResolvedRefAction<'_>,
        ActionRequest,
    ) -> Result<ActionResult, crate::error::AppError>,
) -> Result<ActionResult, AdapterError> {
    let Some(budget_ms) = request.timeout_ms else {
        return execute_single_shot(adapter, entry, ref_id, context, request, dispatch)
            .map_err(|err| enrich_with_process_state(adapter, entry, err));
    };
    execute_poll_loop(
        adapter,
        entry,
        ref_id,
        context,
        request,
        budget_from_ms(budget_ms),
        dispatch,
    )
    .map_err(|err| enrich_with_process_state(adapter, entry, err))
}

fn execute_single_shot(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    ref_id: &str,
    context: &CommandContext,
    request: ActionRequest,
    dispatch: impl Fn(
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
    dispatch: impl Fn(
        crate::ref_action::ResolvedRefAction<'_>,
        ActionRequest,
    ) -> Result<ActionResult, crate::error::AppError>,
) -> Result<ActionResult, AdapterError> {
    let deadline = Instant::now() + budget;
    let mut last_report: Option<Value> = None;
    let mut saw_ambiguity = false;
    loop {
        match resolve_within_deadline(adapter, entry, deadline) {
            ResolveAttemptOutcome::DeadlinePassed => {
                return Err(actionability_timeout(last_report));
            }
            ResolveAttemptOutcome::Resolved(handle) => {
                let resolved = ResolvedElement::new(adapter, handle);
                maybe_scroll_into_view(adapter, entry, resolved.handle(), &request);
                match dispatch(
                    crate::ref_action::ResolvedRefAction {
                        adapter,
                        entry,
                        handle: resolved.handle(),
                        ref_id,
                        context,
                    },
                    request.clone(),
                ) {
                    Ok(mut result) => {
                        if saw_ambiguity {
                            result = result.with_details(json!({ "transient_ambiguity": true }));
                        }
                        return Ok(result);
                    }
                    Err(err) => {
                        let err = crate::ref_action::into_adapter_error(err);
                        if is_permanent_error(&err.code) {
                            return Err(err);
                        }
                        last_report = err.details.clone();
                        sleep_poll_interval(deadline);
                    }
                }
            }
            ResolveAttemptOutcome::Failed(err) => {
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

#[cfg(test)]
#[path = "ref_action_wait_process_state_tests.rs"]
mod process_state_tests;
