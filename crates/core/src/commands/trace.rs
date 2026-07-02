use crate::{
    context::CommandContext,
    error::AppError,
    refs_store::RefStore,
    session::resolve_active_session,
    trace_read::{ExportOptions, ReadOptions, export_html, read_merged},
};
use serde_json::{Value, json};
use std::path::PathBuf;

const DEFAULT_SHOW_LIMIT: usize = 500;

pub const TRACE_SHOW_DEFAULT_LIMIT: usize = DEFAULT_SHOW_LIMIT;

#[derive(Debug, Clone)]
pub enum TraceAction {
    Show { limit: usize, event: Option<String> },
    Export { limit: usize, out: Option<PathBuf> },
}

pub fn execute(action: TraceAction, context: &CommandContext) -> Result<Value, AppError> {
    match action {
        TraceAction::Show { limit, event } => show(context, limit, event),
        TraceAction::Export { limit, out } => export(context, limit, out),
    }
}

fn resolve_trace_session(context: &CommandContext) -> Result<(String, RefStore), AppError> {
    let session_id = resolve_active_session(context.session_id(), None)?.ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            "No active session for trace command",
            "Run `session start` or pass `--session <id>`.",
        )
    })?;
    let store = RefStore::for_session(Some(&session_id))?;
    let trace_dir = store.trace_dir();
    if !trace_dir.is_dir() {
        return Err(AppError::invalid_input_with_suggestion(
            format!("Session '{session_id}' has no trace directory"),
            "Run `session start` with tracing enabled before recording commands.",
        ));
    }
    Ok((session_id, store))
}

fn empty_trace_dir_error(session_id: &str) -> AppError {
    AppError::invalid_input_with_suggestion(
        format!("Session '{session_id}' has an empty trace directory"),
        "Run `session start` with tracing enabled before recording commands.",
    )
}

fn show(context: &CommandContext, limit: usize, event: Option<String>) -> Result<Value, AppError> {
    let (session_id, store) = resolve_trace_session(context)?;
    let trace_dir = store.trace_dir();
    let merged = read_merged(
        &trace_dir,
        &ReadOptions {
            limit,
            event_prefix: event.clone(),
        },
    )?;
    if merged.segments.is_empty() {
        return Err(empty_trace_dir_error(&session_id));
    }

    let mut body = json!({
        "session_id": session_id,
        "segments": merged.segments,
        "returned_events": merged.returned_events,
        "truncated": merged.truncated,
        "events": merged.events,
    });

    match &event {
        Some(prefix) => {
            let unfiltered = read_merged(&trace_dir, &ReadOptions::default())?;
            body["matched_events"] = json!(count_matching_events(&unfiltered.events, prefix));
            body["total_events"] = json!(unfiltered.total_events);
        }
        None => {
            body["total_events"] = json!(merged.total_events);
        }
    }

    if !merged.warnings.is_empty() {
        body["warnings"] = json!(merged.warnings);
    }
    Ok(body)
}

fn count_matching_events(events: &[Value], prefix: &str) -> usize {
    events
        .iter()
        .filter(|event| {
            event
                .get("event")
                .and_then(Value::as_str)
                .is_some_and(|name| name.starts_with(prefix))
        })
        .count()
}

fn export(context: &CommandContext, limit: usize, out: Option<PathBuf>) -> Result<Value, AppError> {
    let (session_id, store) = resolve_trace_session(context)?;
    let (_html, stats) = export_html(
        &store.trace_dir(),
        &session_id,
        &ExportOptions { limit, out },
    )?;
    Ok(json!({
        "path": stats.path,
        "event_count": stats.event_count,
        "screenshots_embedded": stats.screenshots_embedded,
        "screenshots_skipped": stats.screenshots_skipped,
        "bytes": stats.bytes,
    }))
}

#[cfg(test)]
#[path = "trace_tests.rs"]
mod tests;
