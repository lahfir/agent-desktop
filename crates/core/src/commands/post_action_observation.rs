use crate::{
    AdapterError, AppError, Deadline, ProcessIdentity, RefEntry, SignalFilter,
    adapter::PlatformAdapter,
    context::WaitSelector,
    locator::live_locator::{
        LocatorMaterialization, LocatorResolveRequest, LocatorSelection, resolve_query,
    },
    system::process_state::ProcessState,
};
use serde_json::{Value, json};
use std::time::Instant;

#[derive(Clone)]
struct Probe {
    outcome: &'static str,
    selector_matches: Option<bool>,
    process_state: Option<ProcessState>,
    last_error: Option<Value>,
}

enum ProbeResult {
    Selector(bool),
    OwnerUnavailable(ProcessState),
}

pub(crate) fn observe(
    wait: &WaitSelector,
    entry: &RefEntry,
    adapter: &dyn PlatformAdapter,
) -> Value {
    let started = Instant::now();
    let Some(instance) = entry
        .process
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
    else {
        return evidence(wait, entry, &Probe::new("identity_unavailable"), started);
    };
    let process = ProcessIdentity::new(entry.process.pid, instance);
    let query = match crate::commands::query::validate_selector(&wait.query_raw) {
        Ok(query) => query,
        Err(error) => return failed_evidence(wait, entry, error, started),
    };
    let deadline = match Deadline::after(wait.timeout_ms) {
        Ok(deadline) => deadline,
        Err(error) => return failed_evidence(wait, entry, error.into(), started),
    };
    let mut latest = Probe::new("timed_out");
    let result = crate::commands::wait_selector::poll_selector(
        adapter,
        SignalFilter {
            app: entry.source.source_app.clone(),
            process: Some(process.clone()),
        },
        deadline,
        |poll_deadline| {
            Ok(match observe_once(adapter, entry, &query, poll_deadline) {
                Ok(ProbeResult::Selector(matches)) => {
                    latest.selector_matches = Some(matches);
                    latest.process_state = None;
                    latest.last_error = None;
                    latest.outcome = "timed_out";
                    if matches != wait.gone {
                        latest.outcome = "expected_state_observed";
                        Some(latest.clone())
                    } else {
                        None
                    }
                }
                Ok(ProbeResult::OwnerUnavailable(state)) => {
                    latest.selector_matches = None;
                    latest.process_state = Some(state);
                    latest.outcome = match state {
                        ProcessState::Exited { .. } => "process_exited",
                        ProcessState::Crashed { .. } => "process_crashed",
                        _ => "owner_unavailable",
                    };
                    matches!(
                        state,
                        ProcessState::Exited { .. } | ProcessState::Crashed { .. }
                    )
                    .then(|| latest.clone())
                }
                Err(error) => {
                    latest.outcome = "observation_failed";
                    latest.selector_matches = None;
                    latest.last_error = Some(error_value(&error));
                    (deadline.is_expired()
                        || !is_transient(&error, entry.source.source_window_id.as_deref()))
                    .then(|| latest.clone())
                }
            })
        },
    );

    match result {
        Ok(Some(probe)) => evidence(wait, entry, &probe, started),
        Ok(None) => {
            if latest.outcome != "owner_unavailable" {
                latest.outcome = "timed_out";
            }
            evidence(wait, entry, &latest, started)
        }
        Err(error) => failed_evidence(wait, entry, error, started),
    }
}

impl Probe {
    fn new(outcome: &'static str) -> Self {
        Self {
            outcome,
            selector_matches: None,
            process_state: None,
            last_error: None,
        }
    }
}

fn observe_once(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    query: &crate::LocatorQuery,
    deadline: Deadline,
) -> Result<ProbeResult, AppError> {
    use crate::snapshot::ref_owner_observation::RefOwnerObservation;
    let owner = match crate::snapshot::surface_target::resolve_ref_owner(adapter, entry, deadline)?
    {
        RefOwnerObservation::Present(owner) => owner,
        RefOwnerObservation::Absent(None) => return Ok(ProbeResult::Selector(false)),
        RefOwnerObservation::Absent(Some(state)) | RefOwnerObservation::Unavailable(state) => {
            return Ok(ProbeResult::OwnerUnavailable(state));
        }
    };
    let request = LocatorResolveRequest {
        selection: LocatorSelection::First,
        deadline,
        max_raw_depth: 50,
        surface: Some(entry.source.source_surface),
        materialization: LocatorMaterialization::None,
    };
    let observed = resolve_query(
        adapter,
        query,
        owner.observation_root(entry.source.source_surface),
        &request,
    );
    let resolution = match observed {
        Err(error)
            if crate::snapshot::surface_target::is_confirmed_surface_absence(
                &error,
                entry.source.source_surface,
            ) =>
        {
            return Ok(ProbeResult::Selector(false));
        }
        result => result?,
    };
    if resolution.meta.total_matches > 0 {
        return Ok(ProbeResult::Selector(true));
    }
    if resolution.meta.selection_complete {
        Ok(ProbeResult::Selector(false))
    } else {
        Err(AdapterError::new(
            crate::ErrorCode::Timeout,
            "Selector observation was incomplete",
        )
        .into())
    }
}

fn is_transient(error: &AppError, window_id: Option<&str>) -> bool {
    error.code() == crate::ErrorCode::Timeout.as_str()
        || crate::commands::wait_selector::is_transient_poll_error(error, window_id)
}

fn error_value(error: &AppError) -> Value {
    json!({ "code": error.code(), "message": error.to_string() })
}

fn failed_evidence(
    wait: &WaitSelector,
    entry: &RefEntry,
    error: AppError,
    started: Instant,
) -> Value {
    let mut probe = Probe::new("observation_failed");
    probe.last_error = Some(error_value(&error));
    evidence(wait, entry, &probe, started)
}

fn evidence(wait: &WaitSelector, entry: &RefEntry, probe: &Probe, started: Instant) -> Value {
    let mut value = json!({
        "kind": "post_action_observation",
        "selector": wait.query_raw,
        "expected": if wait.gone { "absent" } else { "present" },
        "outcome": probe.outcome,
        "surface": entry.source.source_surface.as_str(),
        "process_identity": {
            "pid": entry.process.pid,
            "process_instance": entry.process.process_instance,
        },
        "elapsed_ms": started.elapsed().as_millis(),
    });
    if let Some(window_id) = &entry.source.source_window_id {
        value["source_window_id"] = json!(window_id);
    }
    if let Some(matches) = probe.selector_matches {
        value["selector_matches"] = json!(matches);
    }
    if let Some(state) = probe.process_state {
        value["process_state"] = json!(state.label());
    }
    if let Some(error) = &probe.last_error {
        value["last_observation_error"] = error.clone();
    }
    value
}

pub(crate) fn attach(mut error: AdapterError, observation: Value) -> AdapterError {
    let mut details = match error.details.take() {
        Some(Value::Object(details)) => details,
        Some(other) => serde_json::Map::from_iter([("adapter_details".into(), other)]),
        None => serde_json::Map::new(),
    };
    details.insert("post_action_observation".into(), observation);
    error.details = Some(Value::Object(details));
    error
}

#[cfg(test)]
#[path = "post_action_observation_tests.rs"]
mod tests;
