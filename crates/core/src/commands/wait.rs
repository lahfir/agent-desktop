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
    pub mode: WaitModeArgs,
    pub predicate: WaitPredicateArgs,
    pub timeout_ms: u64,
    pub app: Option<String>,
}

#[derive(Clone)]
pub struct WaitModeArgs {
    pub ms: Option<u64>,
    pub element: Option<String>,
    pub window: Option<String>,
    pub text: Option<String>,
    pub menu: bool,
    pub menu_closed: bool,
    pub notification: bool,
}

#[derive(Clone)]
pub struct WaitPredicateArgs {
    pub snapshot_id: Option<String>,
    pub predicate: Option<String>,
    pub value: Option<String>,
    pub count: Option<usize>,
}

pub fn execute(
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
        } => wait_for_element(
            ElementWaitInput {
                ref_id,
                snapshot_id,
                predicate,
                timeout_ms,
            },
            adapter,
            context,
        ),
        WaitMode::Window(title) => wait_for_window(title, timeout_ms, adapter),
        WaitMode::Text { text, count, app } => wait_for_text(
            TextWaitInput {
                text,
                expected_count: count,
                app,
                timeout_ms,
            },
            adapter,
            context,
        ),
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

struct ElementWaitInput {
    ref_id: String,
    snapshot_id: Option<String>,
    predicate: wait_predicate::ElementPredicate,
    timeout_ms: u64,
}

fn wait_for_element(
    input: ElementWaitInput,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let ElementWaitInput {
        ref_id,
        snapshot_id,
        predicate,
        timeout_ms,
    } = input;
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let store = RefStore::for_session(context.session_id())?;
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
            let attempt = remaining.min(WAIT_RESOLVE_ATTEMPT);
            match adapter.resolve_element_strict_with_timeout(&entry, attempt) {
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
    let mut last_error = None;

    loop {
        match adapter.list_windows(&filter) {
            Ok(windows) => {
                if let Some(win) = windows.into_iter().find(|w| w.title.contains(&title)) {
                    let elapsed = start.elapsed().as_millis();
                    return Ok(json!({ "found": true, "window": win, "elapsed_ms": elapsed }));
                }
            }
            Err(err) if is_retryable_wait_poll_error(&err.code) => {
                last_error = Some(json!({
                    "code": err.code.as_str(),
                    "message": err.message
                }));
            }
            Err(err) => return Err(AppError::Adapter(err)),
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return wait_timeout::window(&title, timeout_ms, last_error);
        }

        std::thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}

/// Per-attempt cap on ref resolution inside a wait loop, so a slow resolve
/// cannot consume the whole wait budget on the first poll; the predicate is
/// re-checked every attempt across the full timeout.
const WAIT_RESOLVE_ATTEMPT: Duration = Duration::from_millis(750);

fn is_retryable_wait_resolution_error(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::StaleRef
            | ErrorCode::ElementNotFound
            | ErrorCode::AmbiguousTarget
            | ErrorCode::Timeout
    )
}

struct TextWaitInput {
    text: String,
    expected_count: Option<usize>,
    app: Option<String>,
    timeout_ms: u64,
}

fn wait_for_text(
    input: TextWaitInput,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let TextWaitInput {
        text,
        expected_count,
        app,
        timeout_ms,
    } = input;
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let opts = crate::adapter::TreeOptions::default();
    let normalized_text = search_text::normalize(&text);
    let mut interval = Duration::from_millis(200);
    let mut last_error = None;

    loop {
        match snapshot::build(adapter, &opts, app.as_deref(), None) {
            Ok(result) => {
                let matches = wait_text_match::find(&result.tree, &normalized_text, expected_count);
                let matched = expected_count
                    .map(|expected| matches.len() == expected)
                    .unwrap_or_else(|| !matches.is_empty());
                if matched {
                    let snapshot_id = RefStore::for_session(context.session_id())?
                        .save_new_snapshot(&result.refmap)?;
                    let elapsed = start.elapsed().as_millis();
                    let found = matches.first();
                    let mut body = json!({
                        "found": true,
                        "text": text,
                        "ref": found.and_then(|found| found.ref_id.clone()),
                        "role": found.map(|found| found.role.clone()),
                        "snapshot_id": snapshot_id,
                        "elapsed_ms": elapsed
                    });
                    if expected_count.is_some() {
                        body["count"] = json!(matches.len());
                    }
                    return Ok(body);
                }
            }
            Err(err) if is_retryable_wait_app_error(&err) => {
                last_error = Some(json!({
                    "code": err.code(),
                    "message": err.to_string()
                }));
            }
            Err(err) => return Err(err),
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return wait_timeout::text(&text, timeout_ms, expected_count, last_error);
        }

        std::thread::sleep(remaining.min(interval));
        interval = (interval * 2).min(Duration::from_millis(1000));
    }
}

fn is_retryable_wait_poll_error(code: &ErrorCode) -> bool {
    matches!(code, ErrorCode::Timeout | ErrorCode::ElementNotFound)
}

fn is_retryable_wait_app_error(err: &AppError) -> bool {
    matches!(err, AppError::Adapter(err) if is_retryable_wait_poll_error(&err.code))
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
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut baseline_indices: Option<std::collections::HashSet<usize>> = None;
    let mut last_error = None;

    loop {
        match adapter.list_notifications(&filter) {
            Ok(current) => match &baseline_indices {
                None => {
                    baseline_indices = Some(current.iter().map(|n| n.index).collect());
                }
                Some(baseline) => {
                    if let Some(notif) = current.iter().find(|n| !baseline.contains(&n.index)) {
                        let elapsed = start.elapsed().as_millis();
                        return Ok(json!({
                            "condition": "notification",
                            "matched": true,
                            "notification": notif,
                            "elapsed_ms": elapsed,
                        }));
                    }
                }
            },
            Err(err) if is_retryable_wait_poll_error(&err.code) => {
                last_error = Some(json!({
                    "code": err.code.as_str(),
                    "message": err.message
                }));
            }
            Err(err) => return Err(AppError::Adapter(err)),
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return wait_timeout::notification(app.as_ref(), text.as_ref(), timeout_ms, last_error);
        }
        std::thread::sleep(remaining.min(Duration::from_millis(500)));
    }
}

#[cfg(test)]
#[path = "wait_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wait_element_tests.rs"]
mod element_tests;

#[cfg(test)]
#[path = "wait_resolution_tests.rs"]
mod resolution_tests;
