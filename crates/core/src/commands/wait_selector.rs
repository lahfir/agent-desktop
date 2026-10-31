use crate::{
    AppError, ErrorCode,
    adapter::{PlatformAdapter, TreeOptions},
    commands::selector_observation::SelectorObservation,
    commands::{query, snapshot as snapshot_cmd, wait_timeout},
    context::CommandContext,
    refs::refs_store::RefStore,
    snapshot::{self, emit_snapshot_saved},
    system::process_state::ProcessState,
    trace::trace_artifacts,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

const DIAGNOSTIC_SNAPSHOT_BUDGET: Duration = Duration::from_millis(600);

pub struct WaitSelectorInput {
    pub query_raw: String,
    pub gone: bool,
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub opts: TreeOptions,
    pub timeout_ms: u64,
}

pub fn execute(
    input: WaitSelectorInput,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    execute_with_ref(input, None, adapter, context)
}

pub(crate) fn execute_with_ref(
    mut input: WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let query = query::validate_selector(&input.query_raw)?;
    if let Some(entry) = entry {
        input.opts.surface = snapshot::surface_target::ref_observation_surface(adapter, entry);
    }

    let start = Instant::now();
    let deadline = crate::Deadline::at(start, input.timeout_ms)?;
    let mut last_error = None;
    let mut last_built = None;
    let result = poll_selector(
        adapter,
        crate::SignalFilter {
            app: input.app.clone(),
            process: entry.and_then(|entry| entry.process.identity()),
        },
        deadline,
        |poll_deadline| {
            let completion = match observe_selector(adapter, &input, entry, &query, poll_deadline) {
                Ok(Some(true)) if input.gone => None,
                Ok(Some(true)) => {
                    match build_materialization(adapter, &input, entry, &query, poll_deadline) {
                        Ok((true, result)) => {
                            Some(Ok(SelectorCompletion::Matched(Box::new(result))))
                        }
                        Ok((false, result)) => {
                            last_built = Some(result);
                            None
                        }
                        Err(err)
                            if err.code() == ErrorCode::Timeout.as_str()
                                || is_transient_poll_error(&err, input.window_id.as_deref())
                                || is_target_gone_error(&err)
                                || snapshot::surface_target::is_confirmed_surface_absence(
                                    &err,
                                    input.opts.surface,
                                ) =>
                        {
                            last_error = Some(poll_error_json(&err));
                            None
                        }
                        Err(err) => Some(Err(err)),
                    }
                }
                Ok(Some(false)) if input.gone => Some(Ok(SelectorCompletion::Absent)),
                Ok(Some(false)) => None,
                Ok(None) => {
                    last_error = Some(json!({ "kind": "locator_incomplete" }));
                    None
                }
                Err(err) if is_target_gone_error(&err) && input.gone && entry.is_none() => {
                    Some(Ok(SelectorCompletion::Absent))
                }
                Err(err)
                    if err.code() == ErrorCode::Timeout.as_str() && poll_deadline.is_expired() =>
                {
                    return Ok(None);
                }
                Err(err)
                    if err.code() == ErrorCode::Timeout.as_str()
                        || is_transient_poll_error(&err, input.window_id.as_deref())
                        || is_target_gone_error(&err) =>
                {
                    last_error = Some(poll_error_json(&err));
                    None
                }
                Err(err) => Some(Err(err)),
            };
            if let Some(completion) = completion {
                return completion.map(Some);
            }

            let remaining = deadline.remaining();
            if !remaining.is_zero()
                && remaining <= super::wait_scheduler::POLL_INTERVAL
                && last_built.is_none()
            {
                match build_snapshot(adapter, &input, entry, poll_deadline) {
                    Ok(result) => last_built = Some(result),
                    Err(error) => last_error = Some(poll_error_json(&error)),
                }
            }
            Ok(None)
        },
    );
    match result {
        Ok(Some(SelectorCompletion::Matched(result))) => {
            materialized_response(*result, context, &input, start.elapsed().as_millis())
        }
        Ok(Some(SelectorCompletion::Absent)) => Ok(target_absent_response(
            &input.query_raw,
            start.elapsed().as_millis(),
        )),
        Ok(None) => timeout_response(adapter, &input, entry, context, (last_built, last_error)),
        Err(err) if err.code() == ErrorCode::Timeout.as_str() && deadline.is_expired() => {
            timeout_response(
                adapter,
                &input,
                entry,
                context,
                (last_built, Some(poll_error_json(&err))),
            )
        }
        Err(err) => Err(err),
    }
}

enum SelectorCompletion {
    Matched(Box<snapshot::SnapshotResult>),
    Absent,
}

pub(crate) fn poll_selector<T>(
    adapter: &dyn PlatformAdapter,
    filter: crate::SignalFilter,
    deadline: crate::Deadline,
    mut observe: impl FnMut(crate::Deadline) -> Result<Option<T>, AppError>,
) -> Result<Option<T>, AppError> {
    let mut scheduler = super::wait_scheduler::WaitScheduler::new(adapter, &filter, deadline);
    loop {
        if let Some(result) = observe(deadline)? {
            return Ok(Some(result));
        }
        if deadline.is_expired() {
            return Ok(None);
        }
        scheduler.wait(deadline);
        if deadline.is_expired() {
            return Ok(None);
        }
    }
}

fn timeout_response(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    context: &CommandContext,
    previous: (Option<snapshot::SnapshotResult>, Option<Value>),
) -> Result<Value, AppError> {
    let (mut last_built, mut last_error) = previous;
    if last_built.is_none() {
        let diagnostic_deadline =
            crate::Deadline::detached_after(DIAGNOSTIC_SNAPSHOT_BUDGET.as_millis() as u64)?;
        match build_snapshot(adapter, input, entry, diagnostic_deadline) {
            Ok(result) => last_built = Some(result),
            Err(error) if last_error.is_none() => last_error = Some(poll_error_json(&error)),
            Err(_) => {}
        }
    }
    let snapshot_id = persist_last_built(context, last_built.as_ref())?;
    wait_timeout::selector(
        &input.query_raw,
        input.gone,
        input.timeout_ms,
        last_error,
        snapshot_id,
    )
}

fn persist_last_built(
    context: &CommandContext,
    last_built: Option<&snapshot::SnapshotResult>,
) -> Result<Option<String>, AppError> {
    let Some(result) = last_built else {
        return Ok(None);
    };
    let store = RefStore::for_session(context.session_id())?;
    let snapshot_id = store.save_new_snapshot_detached(&result.refmap)?;
    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id, &result.refmap)?;
    Ok(Some(snapshot_id))
}

fn observe_selector(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    query: &crate::LocatorQuery,
    deadline: crate::Deadline,
) -> Result<Option<bool>, AppError> {
    let observation = if let Some(entry) = entry {
        SelectorObservation::observe_ref(adapter, entry, query, deadline)?
    } else {
        SelectorObservation::observe_owners(
            adapter,
            resolve_owners(adapter, input, None, deadline)?,
            input.opts.surface,
            query,
            deadline,
        )?
    };
    match observation {
        SelectorObservation::Selector { matches, complete } => Ok(complete.then_some(matches)),
        SelectorObservation::OwnerUnavailable(
            ProcessState::Exited { .. } | ProcessState::Crashed { .. },
        ) => Ok(Some(false)),
        SelectorObservation::OwnerUnavailable(state) => Err(owner_unavailable(state)),
    }
}

fn build_materialization(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    query: &crate::LocatorQuery,
    deadline: crate::Deadline,
) -> Result<(bool, snapshot::SnapshotResult), AppError> {
    let result = build_snapshot(adapter, input, entry, deadline)?;
    let matched = query::tree_has_match(&result.tree, query);
    Ok((matched, result))
}

fn resolve_owners(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    deadline: crate::Deadline,
) -> Result<Vec<crate::SnapshotOwner>, AppError> {
    if let Some(entry) = entry {
        use snapshot::ref_owner_observation::RefOwnerObservation;
        return match snapshot::surface_target::resolve_ref_owner(adapter, entry, deadline)? {
            RefOwnerObservation::Present(owner) => Ok(vec![owner]),
            RefOwnerObservation::Absent(_) => Ok(Vec::new()),
            RefOwnerObservation::Unavailable(state) => Err(owner_unavailable(state)),
        };
    }
    snapshot::surface_target::resolve_owners(
        adapter,
        input.app.as_deref(),
        input.window_id.as_deref(),
        input.opts.surface,
        deadline,
    )
}

fn owner_unavailable(state: ProcessState) -> AppError {
    crate::AdapterError::app_unresponsive("Ref owner")
        .with_details(json!({
            "kind": "ref_owner_unavailable",
            "retryable": true,
            "process_state": state.label(),
        }))
        .into()
}

fn build_snapshot(
    adapter: &dyn PlatformAdapter,
    input: &WaitSelectorInput,
    entry: Option<&crate::RefEntry>,
    deadline: crate::Deadline,
) -> Result<snapshot::SnapshotResult, AppError> {
    let owners = resolve_owners(adapter, input, entry, deadline)?;
    snapshot::build_for_owners(adapter, &input.opts, owners, deadline)
}

fn materialized_response(
    mut result: snapshot::SnapshotResult,
    context: &CommandContext,
    input: &WaitSelectorInput,
    elapsed_ms: u128,
) -> Result<Value, AppError> {
    let store = RefStore::for_session(context.session_id())?;
    let snapshot_id = store.save_new_snapshot(&result.refmap)?;
    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id, &result.refmap)?;
    result.bind_snapshot_id(snapshot_id);
    emit_snapshot_saved(context, &result)?;
    snapshot_cmd::format_snapshot_fields(&result, Some(elapsed_ms), Some(&input.query_raw))
}

fn target_absent_response(query_raw: &str, elapsed_ms: u128) -> Value {
    json!({
        "matched_selector": query_raw,
        "gone": true,
        "target_absent": true,
        "elapsed_ms": elapsed_ms,
    })
}

fn poll_error_json(err: &AppError) -> Value {
    json!({ "code": err.code(), "message": err.to_string() })
}

fn is_target_gone_error(err: &AppError) -> bool {
    matches!(
        err,
        AppError::Adapter(e)
            if matches!(e.code, ErrorCode::AppNotFound | ErrorCode::WindowNotFound)
    )
}

pub(crate) fn is_transient_poll_error(err: &AppError, window_id: Option<&str>) -> bool {
    let AppError::Adapter(error) = err else {
        return false;
    };
    error.is_explicitly_retryable()
        || (error.code == ErrorCode::ActionNotSupported
            && error.details.as_ref().is_some_and(|details| {
                details["kind"] == "window_without_accessibility_element"
                    && window_id.is_none_or(|id| details["window_id"].as_str() == Some(id))
            }))
}

#[cfg(test)]
#[path = "wait_selector_tests.rs"]
mod tests;
