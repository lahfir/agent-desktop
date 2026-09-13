use crate::{
    AdapterError, AppError, ErrorCode, EventKind, ProcessIdentity, SignalBaseline, SignalFilter,
    UiEvent, adapter::PlatformAdapter, commands::wait_event_input::EventWaitInput, diff_signals,
    process_state::ProcessState, signals::merge_signal_baseline,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// Baseline-diff event wait: captures a `SignalBaseline` at wait start, polls
/// for a fresh one, and matches the first `diff_signals` event whose kind
/// matches the requested `--event` token. `window_id`/`window_title` are
/// optional narrowing filters, never requirements — the whole point of R16
/// is that a caller who does not yet know a new window's id or title can
/// still wait for it to appear.
pub(crate) fn wait_for_event(
    input: EventWaitInput,
    adapter: &dyn PlatformAdapter,
    seeded_baseline: Option<Result<SignalBaseline, AdapterError>>,
) -> Result<Value, AppError> {
    let requested = parse_event_kind(&input.event)?;
    let start = Instant::now();
    let deadline = crate::Deadline::at(start, input.timeout_ms)?;
    let filter = match signal_filter(
        &input,
        &requested,
        seeded_baseline.as_ref(),
        adapter,
        deadline,
    ) {
        Ok(filter) => filter,
        Err(AppError::Adapter(err))
            if err.code == ErrorCode::AppNotFound && is_disappearance_class(&requested) =>
        {
            return unresolved_target_result(&requested, input.app.as_deref(), start);
        }
        Err(err) => return Err(err),
    };
    let disappearance = is_disappearance_class(&requested);
    let (mut baseline, mut last_error) = match seeded_baseline {
        Some(Ok(baseline)) => {
            validate_signal_scope(&filter, &baseline)?;
            (Some(baseline), None)
        }
        Some(Err(error)) if is_retryable(&error.code) => (None, Some(error_evidence(&error))),
        Some(Err(error)) => return Err(AppError::Adapter(error)),
        None => (None, None),
    };
    let mut seen = baseline.clone();

    loop {
        if deadline.is_expired() {
            return timeout_err(
                &input.event,
                input.app.as_ref(),
                input.timeout_ms,
                baseline.as_ref(),
                last_error,
            );
        }

        let observation = adapter.capture_signal_baseline(&filter, deadline);
        if deadline.is_expired() {
            return timeout_err(
                &input.event,
                input.app.as_ref(),
                input.timeout_ms,
                baseline.as_ref(),
                last_error,
            );
        }

        match observation {
            Ok(current) => {
                validate_signal_scope(&filter, &current)?;
                match &baseline {
                    None => {
                        baseline = Some(current.clone());
                        seen = Some(current);
                    }
                    Some(base) => {
                        let diff_base_owned;
                        let diff_base: &SignalBaseline = if disappearance {
                            diff_base_owned = seen.clone().unwrap_or_else(|| base.clone());
                            &diff_base_owned
                        } else {
                            base
                        };
                        let events = diff_signals(diff_base, &current);
                        if let Some(found) = find_match(
                            &events,
                            &requested,
                            input.window_id.as_deref(),
                            input.window_title.as_deref(),
                        ) {
                            if confirm_app_terminated(adapter, found, diff_base, deadline) {
                                let elapsed = start.elapsed().as_millis();
                                return Ok(json!({
                                    "found": true,
                                    "event": serde_json::to_value(found)?,
                                    "elapsed_ms": elapsed,
                                }));
                            }
                        }
                        if disappearance {
                            seen = Some(merge_signal_baseline(diff_base, &current));
                        }
                    }
                }
            }
            Err(err) if is_retryable(&err.code) => {
                last_error = Some(error_evidence(&err));
            }
            Err(err) => return Err(AppError::Adapter(err)),
        }

        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return timeout_err(
                &input.event,
                input.app.as_ref(),
                input.timeout_ms,
                baseline.as_ref(),
                last_error,
            );
        }
        std::thread::sleep(remaining.min(Duration::from_millis(200)));
    }
}

fn is_appearance_class(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::AppLaunched | EventKind::WindowOpened | EventKind::SurfaceAppeared { .. }
    )
}

fn is_disappearance_class(kind: &EventKind) -> bool {
    matches!(
        kind,
        EventKind::AppTerminated | EventKind::WindowClosed | EventKind::SurfaceDismissed { .. }
    )
}

fn unresolved_target_result(
    requested: &EventKind,
    app: Option<&str>,
    start: Instant,
) -> Result<Value, AppError> {
    let event = UiEvent {
        kind: requested.clone(),
        window_id: None,
        title: None,
        app: app.map(str::to_string),
        pid: None,
    };
    Ok(json!({
        "found": true,
        "event": serde_json::to_value(event)?,
        "elapsed_ms": start.elapsed().as_millis(),
    }))
}

/// Before a matched `AppTerminated` is reported, corroborates it against
/// `SystemOps::process_state` so a process that only dropped out of the
/// window-owning population (a close-to-tray) is not mistaken for a real
/// exit. Fails open: any ambiguity — no pid on the event, no recoverable
/// `process_instance`, a `not_supported`/errored read, or a non-`AppTerminated`
/// event — reports the event rather than risking a silent no-op that leaves
/// a genuine termination unreported.
fn confirm_app_terminated(
    adapter: &dyn PlatformAdapter,
    event: &UiEvent,
    diff_base: &SignalBaseline,
    deadline: crate::Deadline,
) -> bool {
    if !matches!(event.kind, EventKind::AppTerminated) {
        return true;
    }
    let Some(pid) = event.pid else {
        return true;
    };
    let Some(instance) = diff_base
        .apps
        .iter()
        .find(|app| app.pid == pid)
        .and_then(|app| app.process_instance.clone())
    else {
        return true;
    };
    match adapter.process_state(ProcessIdentity::new(pid, instance), deadline) {
        Ok(ProcessState::Running | ProcessState::Unresponsive) => false,
        Ok(ProcessState::Exited { .. } | ProcessState::Crashed { .. }) | Err(_) => true,
    }
}

fn signal_filter(
    input: &EventWaitInput,
    requested: &EventKind,
    seeded_baseline: Option<&Result<SignalBaseline, AdapterError>>,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<SignalFilter, AppError> {
    let Some(app) = input.app.as_deref() else {
        return Ok(SignalFilter::default());
    };
    if matches!(requested, EventKind::AppLaunched) {
        return Ok(SignalFilter {
            app: input.app.clone(),
            process: None,
        });
    }
    let successful_seed = seeded_baseline.and_then(|baseline| baseline.as_ref().ok());
    let seeded_process = successful_seed
        .map(|baseline| process_from_baseline(baseline, app))
        .transpose()?
        .flatten();
    if let Some(process) = seeded_process {
        return Ok(SignalFilter {
            app: input.app.clone(),
            process: Some(process),
        });
    }
    if successful_seed.is_some() && matches!(requested, EventKind::AppTerminated) {
        return Ok(SignalFilter {
            app: input.app.clone(),
            process: None,
        });
    }
    match crate::commands::helpers::resolve_app(Some(app), adapter, deadline) {
        Ok(resolved) => Ok(SignalFilter {
            app: input.app.clone(),
            process: Some(crate::commands::helpers::process_identity(&resolved)?),
        }),
        Err(AppError::Adapter(err))
            if err.code == ErrorCode::AppNotFound && is_appearance_class(requested) =>
        {
            Ok(SignalFilter {
                app: input.app.clone(),
                process: None,
            })
        }
        Err(err) => Err(err),
    }
}

fn process_from_baseline(
    baseline: &SignalBaseline,
    app: &str,
) -> Result<Option<crate::ProcessIdentity>, AppError> {
    let mut matches = baseline
        .apps
        .iter()
        .filter(|candidate| candidate.matches_identifier(app));
    let Some(first) = matches.next() else {
        return Ok(None);
    };
    if matches.next().is_some() {
        return Err(AdapterError::ambiguous_target(format!(
            "Multiple application instances matched '{app}' in the event baseline"
        ))
        .into());
    }
    crate::commands::helpers::process_identity(first).map(Some)
}

fn validate_signal_scope(filter: &SignalFilter, baseline: &SignalBaseline) -> Result<(), AppError> {
    let Some(expected) = filter.process.as_ref() else {
        return Ok(());
    };
    let observed = baseline
        .apps
        .iter()
        .map(|app| (app.pid, app.process_instance.as_deref()))
        .chain(
            baseline
                .windows
                .iter()
                .map(|window| (window.pid, window.process_instance.as_deref())),
        )
        .chain(
            baseline
                .surfaces
                .iter()
                .map(|surface| (surface.pid, Some(surface.process_instance.as_str()))),
        );
    if let Some((pid, instance)) = observed.into_iter().find(|(pid, instance)| {
        *pid != expected.pid || *instance != Some(expected.instance.as_str())
    }) {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "Target process identity changed during event observation",
        )
        .with_details(json!({
            "kind": "process_changed",
            "expected_pid": expected.pid,
            "observed_pid": pid,
            "observed_instance_matches": instance == Some(expected.instance.as_str()),
            "retryable": false,
        }))
        .with_disposition(crate::DeliverySemantics::not_delivered())
        .into());
    }
    Ok(())
}

fn find_match<'a>(
    events: &'a [UiEvent],
    requested: &EventKind,
    window_id: Option<&str>,
    window_title: Option<&str>,
) -> Option<&'a UiEvent> {
    events.iter().find(|event| {
        if !requested.same_variant(&event.kind) {
            return false;
        }
        if let Some(id) = window_id {
            if event.window_id.as_deref() != Some(id) {
                return false;
            }
        }
        if let Some(title) = window_title {
            if !title.is_empty() && event.title.as_deref() != Some(title) {
                return false;
            }
        }
        true
    })
}

fn is_retryable(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::Timeout | ErrorCode::ElementNotFound | ErrorCode::AppUnresponsive
    )
}

fn error_evidence(error: &AdapterError) -> Value {
    json!({
        "code": error.code.as_str(),
        "message": error.message,
        "details": error.details,
    })
}

pub(crate) fn parse_event_kind(event: &str) -> Result<EventKind, AppError> {
    EventKind::parse(event).ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            format!("Unknown --event value: {event}"),
            format!("Use one of: {}", EventKind::all_tokens().join(", ")),
        )
    })
}

fn timeout_err(
    event: &str,
    app: Option<&String>,
    timeout_ms: u64,
    baseline: Option<&SignalBaseline>,
    last_error: Option<Value>,
) -> Result<Value, AppError> {
    let mut details = json!({
        "kind": "wait_timeout",
        "predicate": "event",
        "event": event,
        "app": app,
        "timeout_ms": timeout_ms,
        "baseline_counts": baseline.map(|base| json!({
            "windows": base.windows.len(),
            "apps": base.apps.len(),
            "surfaces": base.surfaces.len(),
        })),
    });
    if let Some(err) = last_error {
        details["last_error"] = err;
    }
    Err(AppError::Adapter(
        AdapterError::timeout(format!(
            "Event '{event}' did not occur within {timeout_ms}ms"
        ))
        .with_details(details),
    ))
}

#[cfg(test)]
#[path = "wait_event_tests.rs"]
mod tests;
