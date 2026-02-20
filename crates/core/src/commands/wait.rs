use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    commands::helpers::{resolve_app_pid, validate_ref_id},
    error::AppError,
    node::AccessibilityNode,
    refs::RefMap,
    snapshot,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub struct WaitArgs {
    pub ms: Option<u64>,
    pub element: Option<String>,
    pub window: Option<String>,
    pub text: Option<String>,
    pub timeout_ms: u64,
    pub menu: bool,
    pub menu_closed: bool,
    pub app: Option<String>,
}

pub fn execute(args: WaitArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if let Some(ms) = args.ms {
        std::thread::sleep(Duration::from_millis(ms));
        return Ok(json!({ "waited_ms": ms }));
    }

    if args.menu || args.menu_closed {
        let pid = resolve_app_pid(args.app.as_deref(), adapter)?;
        let start = Instant::now();
        adapter.wait_for_menu(pid, args.menu, args.timeout_ms).map_err(AppError::Adapter)?;
        let elapsed = start.elapsed().as_millis();
        return Ok(json!({ "found": true, "elapsed_ms": elapsed }));
    }

    if let Some(ref_id) = args.element {
        validate_ref_id(&ref_id)?;
        return wait_for_element(ref_id, args.timeout_ms, adapter);
    }

    if let Some(title) = args.window {
        return wait_for_window(title, args.timeout_ms, adapter);
    }

    if let Some(text) = args.text {
        return wait_for_text(text, args.app, args.timeout_ms, adapter);
    }

    Err(AppError::invalid_input(
        "Provide a duration (ms), --menu, --element <ref>, --window <title>, or --text <text>",
    ))
}

fn wait_for_element(
    ref_id: String,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);

    loop {
        if let Ok(refmap) = RefMap::load() {
            if let Some(entry) = refmap.get(&ref_id) {
                if adapter.resolve_element(entry).is_ok() {
                    let elapsed = start.elapsed().as_millis();
                    return Ok(json!({ "found": true, "ref": ref_id, "elapsed_ms": elapsed }));
                }
            }
        }

        if start.elapsed() >= timeout {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(format!(
                "Element {ref_id} not found within {timeout_ms}ms"
            ))));
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_window(
    title: String,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let filter = WindowFilter { focused_only: false, app: None };

    loop {
        if let Ok(windows) = adapter.list_windows(&filter) {
            if let Some(win) = windows.into_iter().find(|w| w.title.contains(&title)) {
                let elapsed = start.elapsed().as_millis();
                return Ok(json!({ "found": true, "window": win, "elapsed_ms": elapsed }));
            }
        }

        if start.elapsed() >= timeout {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(format!(
                "Window with title '{title}' not found within {timeout_ms}ms"
            ))));
        }

        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_text(
    text: String,
    app: Option<String>,
    timeout_ms: u64,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let opts = crate::adapter::TreeOptions::default();
    let text_lower = text.to_lowercase();

    loop {
        if let Ok(result) = snapshot::run(adapter, &opts, app.as_deref(), None) {
            if let Some(found) = find_text_in_tree(&result.tree, &text_lower) {
                let elapsed = start.elapsed().as_millis();
                return Ok(json!({
                    "found": true,
                    "text": text,
                    "ref": found.ref_id,
                    "role": found.role,
                    "elapsed_ms": elapsed
                }));
            }
        }

        if start.elapsed() >= timeout {
            return Err(AppError::Adapter(crate::error::AdapterError::timeout(format!(
                "Text '{text}' not found within {timeout_ms}ms"
            ))));
        }

        std::thread::sleep(Duration::from_millis(200));
    }
}

struct TextMatch {
    ref_id: Option<String>,
    role: String,
}

fn find_text_in_tree(node: &AccessibilityNode, text_lower: &str) -> Option<TextMatch> {
    let in_name = node.name.as_deref().is_some_and(|n| n.to_lowercase().contains(text_lower));
    let in_value = node.value.as_deref().is_some_and(|v| v.to_lowercase().contains(text_lower));
    let in_desc = node.description.as_deref().is_some_and(|d| d.to_lowercase().contains(text_lower));

    if in_name || in_value || in_desc {
        return Some(TextMatch { ref_id: node.ref_id.clone(), role: node.role.clone() });
    }

    for child in &node.children {
        if let Some(found) = find_text_in_tree(child, text_lower) {
            return Some(found);
        }
    }
    None
}
