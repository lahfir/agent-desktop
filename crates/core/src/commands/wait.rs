use crate::{
    adapter::{PlatformAdapter, WindowFilter},
    commands::helpers::validate_ref_id,
    error::AppError,
    refs::RefMap,
};
use serde_json::{json, Value};
use std::time::{Duration, Instant};

pub struct WaitArgs {
    pub ms: Option<u64>,
    pub element: Option<String>,
    pub window: Option<String>,
    pub timeout_ms: u64,
}

pub fn execute(args: WaitArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if let Some(ms) = args.ms {
        std::thread::sleep(Duration::from_millis(ms));
        return Ok(json!({ "waited_ms": ms }));
    }

    if let Some(ref_id) = args.element {
        validate_ref_id(&ref_id)?;
        return wait_for_element(ref_id, args.timeout_ms, adapter);
    }

    if let Some(title) = args.window {
        return wait_for_window(title, args.timeout_ms, adapter);
    }

    Err(AppError::invalid_input(
        "Provide a duration (ms), --element <ref>, or --window <title>",
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
            if refmap.get(&ref_id).is_some()
                && adapter.resolve_element(refmap.get(&ref_id).unwrap()).is_ok()
            {
                let elapsed = start.elapsed().as_millis();
                return Ok(json!({ "found": true, "ref": ref_id, "elapsed_ms": elapsed }));
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
