use crate::{
    adapter::PlatformAdapter,
    error::{AdapterError, AppError, ErrorCode},
    signals::{self, EventKind, SignalBaseline, SignalFilter, UiEvent},
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
    event: &str,
    app: Option<String>,
    window_id: Option<String>,
    window_title: Option<String>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let requested = parse_event_kind(event)?;
    let filter = SignalFilter {
        app: app.clone(),
        pid: None,
    };
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut baseline: Option<SignalBaseline> = None;
    let mut last_error = None;

    loop {
        match adapter.capture_signal_baseline(&filter) {
            Ok(current) => match &baseline {
                None => baseline = Some(current),
                Some(base) => {
                    let events = signals::diff_signals(base, &current);
                    if let Some(found) = find_match(
                        &events,
                        &requested,
                        window_id.as_deref(),
                        window_title.as_deref(),
                    ) {
                        let elapsed = start.elapsed().as_millis();
                        return Ok(json!({
                            "found": true,
                            "event": serde_json::to_value(found)?,
                            "elapsed_ms": elapsed,
                        }));
                    }
                }
            },
            Err(err) if is_retryable(&err.code) => {
                last_error = Some(json!({
                    "code": err.code.as_str(),
                    "message": err.message
                }));
            }
            Err(err) => return Err(AppError::Adapter(err)),
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return timeout_err(
                event,
                app.as_ref(),
                timeout_ms,
                baseline.as_ref(),
                last_error,
            );
        }
        std::thread::sleep(remaining.min(Duration::from_millis(200)));
    }
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
    matches!(code, ErrorCode::Timeout | ErrorCode::ElementNotFound)
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
