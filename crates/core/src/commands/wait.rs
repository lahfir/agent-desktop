use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    commands::{
        helpers::resolve_app_pid,
        wait_element::{ElementWaitInput, wait_for_element},
        wait_mode::WaitMode,
        wait_text_match, wait_timeout,
    },
    context::CommandContext,
    error::{AppError, ErrorCode},
    notification::{NotificationFilter, NotificationInfo},
    refs_store::RefStore,
    search_text,
    snapshot::{self, emit_snapshot_saved},
    trace_artifacts,
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
    pub action: Option<String>,
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
                    let store = RefStore::for_session(context.session_id())?;
                    let snapshot_id = store.save_new_snapshot(&result.refmap)?;
                    trace_artifacts::copy_refmap_if_full(
                        context,
                        &store,
                        &snapshot_id,
                        &result.refmap,
                    )?;
                    emit_snapshot_saved(context, &result)?;
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
    let mut baseline: Option<std::collections::HashMap<NotificationFingerprint, usize>> = None;
    let mut last_error = None;

    loop {
        match adapter.list_notifications(&filter) {
            Ok(current) => match &baseline {
                None => {
                    baseline = Some(notification_counts(&current));
                }
                Some(baseline) => {
                    if let Some(notif) = first_new_notification(&current, baseline) {
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

#[derive(Clone, Eq, Hash, PartialEq)]
struct NotificationFingerprint {
    app_name: String,
    title: String,
    body: Option<String>,
    actions: Vec<String>,
}

impl From<&NotificationInfo> for NotificationFingerprint {
    fn from(info: &NotificationInfo) -> Self {
        Self {
            app_name: info.app_name.clone(),
            title: info.title.clone(),
            body: info.body.clone(),
            actions: info.actions.clone(),
        }
    }
}

fn notification_counts(
    notifications: &[NotificationInfo],
) -> std::collections::HashMap<NotificationFingerprint, usize> {
    let mut counts = std::collections::HashMap::new();
    for notification in notifications {
        *counts
            .entry(NotificationFingerprint::from(notification))
            .or_insert(0) += 1;
    }
    counts
}

fn first_new_notification<'a>(
    current: &'a [NotificationInfo],
    baseline: &std::collections::HashMap<NotificationFingerprint, usize>,
) -> Option<&'a NotificationInfo> {
    let mut seen = std::collections::HashMap::new();
    for notification in current {
        let fingerprint = NotificationFingerprint::from(notification);
        let current_count = seen.entry(fingerprint.clone()).or_insert(0);
        *current_count += 1;
        if *current_count > baseline.get(&fingerprint).copied().unwrap_or(0) {
            return Some(notification);
        }
    }
    None
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
#[path = "wait_predicate_tests.rs"]
mod predicate_tests;

#[cfg(test)]
#[path = "wait_resolution_tests.rs"]
mod resolution_tests;
