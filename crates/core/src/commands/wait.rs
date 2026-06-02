use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    commands::{helpers::resolve_app_pid, wait_latest_ref_cache::LatestRefCache, wait_predicate},
    context::CommandContext,
    error::{AppError, ErrorCode},
    node::AccessibilityNode,
    notification::NotificationFilter,
    refs::validate_ref_id,
    refs_store::RefStore,
    search_text, snapshot,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

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

pub fn execute(args: WaitArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    execute_with_context(args, adapter, &CommandContext::default())
}

pub fn execute_with_context(
    args: WaitArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    validate_wait_mode(&args)?;

    if let Some(ms) = args.ms {
        std::thread::sleep(Duration::from_millis(ms));
        return Ok(json!({ "waited_ms": ms }));
    }

    if args.menu || args.menu_closed {
        let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
        let start = Instant::now();
        adapter
            .wait_for_menu(pid, args.menu, args.timeout_ms)
            .map_err(AppError::Adapter)?;
        let elapsed = start.elapsed().as_millis();
        return Ok(json!({ "found": true, "elapsed_ms": elapsed }));
    }

    if args.notification {
        return wait_for_notification(&args, adapter);
    }

    if let Some(ref_id) = args.element {
        validate_ref_id(&ref_id)?;
        let predicate =
            wait_predicate::ElementPredicate::parse(args.predicate.as_deref(), args.value)?;
        return wait_for_element(
            ref_id,
            args.snapshot_id,
            predicate,
            args.timeout_ms,
            adapter,
            context,
        );
    }

    if let Some(title) = args.window {
        return wait_for_window(title, args.timeout_ms, adapter);
    }

    if let Some(text) = args.text {
        return wait_for_text(
            text,
            args.count,
            args.app,
            args.timeout_ms,
            adapter,
            context,
        );
    }

    Err(AppError::invalid_input(
        "Provide a duration (ms), --menu, --notification, --element <ref>, --window <title>, or --text <text>",
    ))
}

fn validate_wait_mode(args: &WaitArgs) -> Result<(), AppError> {
    if args.predicate.is_some() && args.element.is_none() {
        return Err(AppError::invalid_input_with_suggestion(
            "--predicate requires --element",
            "Use --element <ref> with --predicate, or remove --predicate.",
        ));
    }
    if args.value.is_some() && args.element.is_none() {
        return Err(AppError::invalid_input_with_suggestion(
            "--value requires --element and --predicate value",
            "Use --element <ref> --predicate value --value <expected>.",
        ));
    }
    if args.count.is_some() && args.text.is_none() {
        return Err(AppError::invalid_input_with_suggestion(
            "--count requires --text",
            "Use --text <text> --count <expected>, or remove --count.",
        ));
    }
    let selected = [
        args.ms.is_some(),
        args.element.is_some(),
        args.window.is_some(),
        args.text.is_some() && !args.notification,
        args.menu,
        args.menu_closed,
        args.notification,
    ]
    .into_iter()
    .filter(|selected| *selected)
    .count();
    if selected <= 1 {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "wait accepts exactly one mode",
        "Use one of: ms, --element, --window, --text, --menu, --menu-closed, or --notification.",
    ))
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

    let last_observed = json!(null);
    loop {
        let entry = fixed_refmap
            .as_ref()
            .and_then(|r| r.get(&ref_id).cloned())
            .or_else(|| latest_cache.as_ref().and_then(|c| c.entry(&ref_id)));
        if let Some(entry) = entry {
            match adapter.resolve_element_strict(&entry) {
                Ok(handle) => {
                    let observed = wait_predicate::matches(&entry, &handle, &predicate, adapter);
                    let _ = adapter.release_handle(&handle);
                    if let Some(observed) = observed {
                        let elapsed = start.elapsed().as_millis();
                        return Ok(json!({
                            "found": true,
                            "ref": ref_id,
                            "predicate": predicate.name(),
                            "observed": observed,
                            "elapsed_ms": elapsed
                        }));
                    }
                }
                Err(err) if fixed_refmap.is_none() && err.code == ErrorCode::StaleRef => {
                    if let Some(cache) = latest_cache.as_mut() {
                        cache.refresh_if_due();
                    }
                }
                Err(_) => {}
            }
        } else if let Some(cache) = latest_cache.as_mut() {
            cache.refresh_if_due();
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(
                format!(
                    "Element {ref_id} did not satisfy predicate '{}' within {timeout_ms}ms; last_observed={last_observed}",
                    predicate.name()
                ),
            )));
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

        if start.elapsed() >= timeout {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(
                format!("Window with title '{title}' not found within {timeout_ms}ms"),
            )));
        }

        std::thread::sleep(Duration::from_millis(100));
    }
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
            let matches = find_all_text_in_tree(&result.tree, &normalized_text);
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
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(
                format!("Text '{text}' did not match within {timeout_ms}ms"),
            )));
        }

        std::thread::sleep(remaining.min(interval));
        interval = (interval * 2).min(Duration::from_millis(1000));
    }
}

struct TextMatch {
    ref_id: Option<String>,
    role: String,
}

fn wait_for_notification(
    args: &WaitArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let filter = NotificationFilter {
        app: args.app.clone(),
        text: args.text.clone(),
        ..Default::default()
    };
    let baseline = adapter
        .list_notifications(&filter)
        .map_err(AppError::Adapter)?;
    let baseline_indices: std::collections::HashSet<usize> =
        baseline.iter().map(|n| n.index).collect();
    let interval = Duration::from_millis(500);
    let deadline = Instant::now() + Duration::from_millis(args.timeout_ms);
    let start = Instant::now();

    loop {
        if Instant::now() > deadline {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(
                format!("No new notification within {}ms", args.timeout_ms),
            )));
        }
        let current = adapter
            .list_notifications(&filter)
            .map_err(AppError::Adapter)?;
        let Some(notif) = current
            .iter()
            .find(|n| !baseline_indices.contains(&n.index))
        else {
            std::thread::sleep(interval);
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

fn find_all_text_in_tree(node: &AccessibilityNode, text_lower: &str) -> Vec<TextMatch> {
    let mut matches = Vec::new();
    collect_text_matches(node, text_lower, &mut matches);
    matches
}

fn collect_text_matches(node: &AccessibilityNode, text_lower: &str, matches: &mut Vec<TextMatch>) {
    if search_text::node_contains(node, text_lower) {
        matches.push(TextMatch {
            ref_id: node.ref_id.clone(),
            role: node.role.clone(),
        });
    }

    for child in &node.children {
        collect_text_matches(child, text_lower, matches);
    }
}

#[cfg(test)]
#[path = "wait_tests.rs"]
mod tests;
