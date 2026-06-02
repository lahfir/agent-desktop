use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    commands::{
        helpers::resolve_app_pid, wait_latest_ref_cache::LatestRefCache, wait_mode::WaitMode,
        wait_predicate, wait_text_match, wait_timeout,
    },
    context::CommandContext,
    error::{AppError, ErrorCode},
    notification::NotificationFilter,
    refs_store::RefStore,
    search_text, snapshot,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

#[cfg(test)]
use crate::commands::wait_mode::validate_wait_mode;

#[derive(Clone)]
pub struct WaitArgs {
    pub ms: Option<u64>,
    pub element: Option<String>,
    pub snapshot_id: Option<String>,
    pub predicate: Option<String>,
    pub value: Option<String>,
    pub count: Option<usize>,
    pub window: Option<String>,
    pub text: Option<String>,
    pub timeout_ms: u64,
    pub menu: bool,
    pub menu_closed: bool,
    pub notification: bool,
    pub app: Option<String>,
}

#[cfg(test)]
pub fn execute(args: WaitArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    execute_with_context(args, adapter, &CommandContext::default())
}

pub fn execute_with_context(
    args: WaitArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let timeout_ms = args.timeout_ms;
    match WaitMode::from_args(args)? {
        WaitMode::Sleep(ms) => {
            std::thread::sleep(Duration::from_millis(ms));
            Ok(json!({ "waited_ms": ms }))
        }
        WaitMode::Menu { app, open } => wait_for_menu(app, open, timeout_ms, adapter),
        WaitMode::Notification { app, text } => {
            wait_for_notification(app, text, timeout_ms, adapter)
        }
        WaitMode::Element {
            ref_id,
            snapshot_id,
            predicate,
        } => wait_for_element(ref_id, snapshot_id, predicate, timeout_ms, adapter, context),
        WaitMode::Window(title) => wait_for_window(title, timeout_ms, adapter),
        WaitMode::Text { text, count, app } => {
            wait_for_text(text, count, app, timeout_ms, adapter, context)
        }
    }
}

fn wait_for_menu(
    app: Option<String>,
    open: bool,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let pid = resolve_app_pid(app.as_deref(), adapter)?;
    let start = Instant::now();
    adapter
        .wait_for_menu(pid, open, timeout_ms)
        .map_err(AppError::Adapter)?;
    let elapsed = start.elapsed().as_millis();
    Ok(json!({ "found": true, "elapsed_ms": elapsed }))
}

fn wait_for_element(
    ref_id: String,
    snapshot_id: Option<String>,
    predicate: wait_predicate::ElementPredicate,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let store = RefStore::for_session(context.session_id.as_deref())?;
    let fixed_refmap = match snapshot_id.as_deref() {
        Some(id) => Some(store.load_snapshot(id)?),
        None => None,
    };
    let mut latest_cache = if fixed_refmap.is_none() {
        Some(LatestRefCache::new(&store)?)
    } else {
        None
    };

    if fixed_refmap
        .as_ref()
        .is_some_and(|refmap| refmap.get(&ref_id).is_none())
    {
        return Err(AppError::invalid_input_with_suggestion(
            format!("Ref {ref_id} is not present in the requested snapshot"),
            "Use a ref returned by that snapshot_id, or omit --snapshot to wait against the latest refmap.",
        ));
    }

    let mut last_observed = json!(null);
    loop {
        let entry = fixed_refmap
            .as_ref()
            .and_then(|r| r.get(&ref_id).cloned())
            .or_else(|| latest_cache.as_ref().and_then(|c| c.entry(&ref_id)));
        if let Some(entry) = entry {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return wait_timeout::element(ref_id, predicate, timeout_ms, last_observed);
            }
            match adapter.resolve_element_strict_with_timeout(&entry, remaining) {
                Ok(handle) => {
                    let observed = wait_predicate::observe(&entry, &handle, &predicate, adapter);
                    let _ = adapter.release_handle(&handle);
                    last_observed = observed.map_err(AppError::Adapter)?;
                    if wait_predicate::satisfied(&predicate, &last_observed) {
                        let elapsed = start.elapsed().as_millis();
                        return Ok(json!({
                            "found": true,
                            "ref": ref_id,
                            "predicate": predicate.name(),
                            "observed": last_observed,
                            "elapsed_ms": elapsed
                        }));
                    }
                }
                Err(err) if is_retryable_wait_resolution_error(&err.code) => {
                    last_observed = json!({
                        "error": err.code.as_str(),
                        "message": err.message
                    });
                    if fixed_refmap.is_none() {
                        if let Some(cache) = latest_cache.as_mut() {
                            cache.refresh_if_due();
                        }
                    }
                }
                Err(err) => return Err(AppError::Adapter(err)),
            }
        } else if let Some(cache) = latest_cache.as_mut() {
            cache.refresh_if_due();
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return wait_timeout::element(ref_id, predicate, timeout_ms, last_observed);
        }
        std::thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}
fn wait_for_window(
    title: String,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };

    loop {
        if let Ok(windows) = adapter.list_windows(&filter) {
            if let Some(win) = windows.into_iter().find(|w| w.title.contains(&title)) {
                let elapsed = start.elapsed().as_millis();
                return Ok(json!({ "found": true, "window": win, "elapsed_ms": elapsed }));
            }
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(AppError::Adapter(
                crate::error::AdapterError::timeout(format!(
                    "Window with title '{title}' not found within {timeout_ms}ms"
                ))
                .with_details(json!({
                    "predicate": "window",
                    "title": title,
                    "timeout_ms": timeout_ms
                })),
            ));
        }

        std::thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}

fn is_retryable_wait_resolution_error(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::StaleRef
            | ErrorCode::ElementNotFound
            | ErrorCode::AmbiguousTarget
            | ErrorCode::Timeout
    )
}

fn wait_for_text(
    text: String,
    expected_count: Option<usize>,
    app: Option<String>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let opts = crate::adapter::TreeOptions::default();
    let normalized_text = search_text::normalize(&text);
    let mut interval = Duration::from_millis(200);

    loop {
        if let Ok(result) = snapshot::build(adapter, &opts, app.as_deref(), None) {
            let matches = wait_text_match::find(&result.tree, &normalized_text, expected_count);
            let matched = expected_count
                .map(|expected| matches.len() == expected)
                .unwrap_or_else(|| !matches.is_empty());
            if matched {
                let snapshot_id = RefStore::for_session(context.session_id.as_deref())?
                    .save_new_snapshot(&result.refmap)?;
                let elapsed = start.elapsed().as_millis();
                let found = matches.first();
                return Ok(json!({
                    "found": true,
                    "text": text,
                    "ref": found.and_then(|found| found.ref_id.clone()),
                    "role": found.map(|found| found.role.clone()),
                    "count": matches.len(),
                    "snapshot_id": snapshot_id,
                    "elapsed_ms": elapsed
                }));
            }
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(AppError::Adapter(
                crate::error::AdapterError::timeout(format!(
                    "Text '{text}' did not match within {timeout_ms}ms"
                ))
                .with_details(json!({
                    "predicate": "text",
                    "text_chars": text.chars().count(),
                    "timeout_ms": timeout_ms,
                    "expected_count": expected_count
                })),
            ));
        }

        std::thread::sleep(remaining.min(interval));
        interval = (interval * 2).min(Duration::from_millis(1000));
    }
}

fn wait_for_notification(
    app: Option<String>,
    text: Option<String>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let filter = NotificationFilter {
        app: app.clone(),
        text: text.clone(),
        ..Default::default()
    };
    let baseline = adapter
        .list_notifications(&filter)
        .map_err(AppError::Adapter)?;
    let baseline_indices: std::collections::HashSet<usize> =
        baseline.iter().map(|n| n.index).collect();
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    loop {
        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(AppError::Adapter(
                crate::error::AdapterError::timeout(format!(
                    "No new notification within {timeout_ms}ms"
                ))
                .with_details(json!({
                    "predicate": "notification",
                    "timeout_ms": timeout_ms,
                    "app": app.clone(),
                    "text_chars": text.as_ref().map(|text| text.chars().count())
                })),
            ));
        }
        let current = adapter
            .list_notifications(&filter)
            .map_err(AppError::Adapter)?;
        let Some(notif) = current
            .iter()
            .find(|n| !baseline_indices.contains(&n.index))
        else {
            std::thread::sleep(remaining.min(Duration::from_millis(500)));
            continue;
        };
        let elapsed = start.elapsed().as_millis();
        return Ok(json!({
            "condition": "notification",
            "matched": true,
            "notification": notif,
            "elapsed_ms": elapsed,
        }));
    }
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wait_element_tests.rs"]
mod element_tests;

#[cfg(test)]
#[path = "wait_resolution_tests.rs"]
mod resolution_tests;
