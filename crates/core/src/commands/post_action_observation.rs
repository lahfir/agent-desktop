use crate::{
    AdapterError, AppError, Deadline, RefEntry, SignalFilter, adapter::PlatformAdapter,
    commands::selector_observation::SelectorObservation, context::WaitSelector,
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

pub(crate) fn observe(
    wait: &WaitSelector,
    entry: &RefEntry,
    adapter: &dyn PlatformAdapter,
) -> Value {
    let started = Instant::now();
    let surface = crate::snapshot::surface_target::ref_observation_surface(adapter, entry);
    let Some(process) = entry.process.identity() else {
        return evidence(
            wait,
            entry,
            surface,
            &Probe::new("identity_unavailable"),
            started,
        );
    };
    let query = match crate::commands::query::validate_selector(&wait.query_raw) {
        Ok(query) => query,
        Err(error) => return failed_evidence(wait, entry, surface, error, started),
    };
    let deadline = match Deadline::after(wait.timeout_ms) {
        Ok(deadline) => deadline,
        Err(error) => return failed_evidence(wait, entry, surface, error.into(), started),
    };
    let mut latest = Probe::new("timed_out");
    let result = crate::commands::wait_selector::poll_selector(
        adapter,
        SignalFilter {
            app: entry.source.source_app.clone(),
            process: Some(process),
        },
        deadline,
        |poll_deadline| {
            let observed = SelectorObservation::observe_ref(adapter, entry, &query, poll_deadline);
            if matches!(&observed, Err(error)
                if error.code() == crate::ErrorCode::Timeout.as_str() && poll_deadline.is_expired())
            {
                return Ok(None);
            }
            latest = Probe::new("timed_out");
            Ok(match observed {
                Ok(SelectorObservation::Selector { matches, complete }) if matches || complete => {
                    latest.selector_matches = Some(matches);
                    if matches != wait.gone {
                        latest.outcome = "expected_state_observed";
                        Some(latest.clone())
                    } else {
                        None
                    }
                }
                Ok(SelectorObservation::Selector { .. }) => {
                    latest.last_error = Some(error_value(
                        &AdapterError::new(
                            crate::ErrorCode::Timeout,
                            "Selector observation was incomplete",
                        )
                        .into(),
                    ));
                    None
                }
                Ok(SelectorObservation::OwnerUnavailable(state)) => {
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
                    let transient = is_transient(&error, entry.source.source_window_id.as_deref());
                    latest.outcome = if transient {
                        "timed_out"
                    } else {
                        "observation_failed"
                    };
                    latest.last_error = Some(error_value(&error));
                    (deadline.is_expired() || !transient).then(|| latest.clone())
                }
            })
        },
    );

    match result {
        Ok(Some(probe)) => evidence(wait, entry, surface, &probe, started),
        Ok(None) => {
            if latest.outcome != "owner_unavailable" {
                latest.outcome = "timed_out";
            }
            evidence(wait, entry, surface, &latest, started)
        }
        Err(error) => failed_evidence(wait, entry, surface, error, started),
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
    surface: crate::SnapshotSurface,
    error: AppError,
    started: Instant,
) -> Value {
    let mut probe = Probe::new(
        if is_transient(&error, entry.source.source_window_id.as_deref()) {
            "timed_out"
        } else {
            "observation_failed"
        },
    );
    probe.last_error = Some(error_value(&error));
    evidence(wait, entry, surface, &probe, started)
}

fn evidence(
    wait: &WaitSelector,
    entry: &RefEntry,
    surface: crate::SnapshotSurface,
    probe: &Probe,
    started: Instant,
) -> Value {
    let mut value = json!({
        "kind": "post_action_observation",
        "selector": wait.query_raw,
        "expected": if wait.gone { "absent" } else { "present" },
        "outcome": probe.outcome,
        "surface": surface.as_str(),
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
