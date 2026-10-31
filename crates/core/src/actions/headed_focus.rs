use serde_json::json;

use crate::{AdapterError, AppError, ErrorCode, PlatformAdapter, ProcessIdentity, WindowInfo};

pub(crate) fn focus_entry_window(
    entry: &crate::RefEntry,
    adapter: &dyn PlatformAdapter,
    context: &crate::CommandContext,
    lease: &crate::InteractionLease,
) -> Result<WindowInfo, AppError> {
    let expected = adapter.resolve_ref_focus_window(entry, lease.deadline())?;
    focus_exact_window(&expected, adapter, context, lease)
}

pub fn saved_ref_focus_window(entry: &crate::RefEntry) -> Result<WindowInfo, AdapterError> {
    let process = entry.process.identity().ok_or_else(|| {
        AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Headed input requires target process-instance identity",
        )
        .with_details(json!({ "physical_delivery_started": false }))
    })?;
    let window_id = required_text(
        entry.source.source_window_id.as_deref(),
        "Headed input requires an exact source window id",
    )?;
    let expected = WindowInfo {
        id: window_id.to_string(),
        title: entry.source.source_window_title.clone().unwrap_or_default(),
        app: entry.source.source_app.clone().unwrap_or_default(),
        pid: process.pid,
        process_instance: Some(process.instance),
        bounds: None,
        state: crate::WindowState::default(),
    };
    Ok(expected)
}

pub(crate) fn focus_process_window(
    process: ProcessIdentity,
    adapter: &dyn PlatformAdapter,
    context: &crate::CommandContext,
    lease: &crate::InteractionLease,
) -> Result<WindowInfo, AppError> {
    let expected =
        crate::system::window_lookup::find_window_for_process(process, adapter, lease.deadline())?;
    focus_exact_window(&expected, adapter, context, lease)
}

pub(crate) fn focus_exact_window(
    expected: &WindowInfo,
    adapter: &dyn PlatformAdapter,
    context: &crate::CommandContext,
    lease: &crate::InteractionLease,
) -> Result<WindowInfo, AppError> {
    let live = adapter.resolve_window_strict(expected, lease.deadline())?;
    if live.id != expected.id
        || live.pid != expected.pid
        || live.process_instance != expected.process_instance
    {
        return Err(AdapterError::stale_ref_because(
            "Headed target window belongs to a different process instance",
        )
        .into());
    }
    adapter.focus_window(&live, lease)?;
    context.trace_lazy(
        "input.focus_window",
        || json!({ "pid": live.pid, "window_id": live.id, "ok": true }),
    )?;
    Ok(live)
}

fn required_text<'a>(value: Option<&'a str>, message: &str) -> Result<&'a str, AdapterError> {
    value.filter(|value| !value.is_empty()).ok_or_else(|| {
        AdapterError::new(ErrorCode::ActionNotSupported, message)
            .with_details(json!({ "physical_delivery_started": false }))
    })
}

#[cfg(test)]
#[path = "headed_focus_tests.rs"]
mod tests;
