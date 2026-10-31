use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ObservationOps, RefEntry, SnapshotSurface, WindowInfo,
};

use crate::adapter::MacOSAdapter;

pub(crate) fn resolve(
    adapter: &MacOSAdapter,
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<WindowInfo, AdapterError> {
    if entry.source.source_window_id.is_some()
        || entry.source.source_surface != SnapshotSurface::Menu
    {
        return agent_desktop_core::saved_ref_focus_window(entry);
    }
    let instance = entry
        .process
        .process_instance
        .as_deref()
        .filter(|value| !value.is_empty())
        .ok_or_else(missing_owner)?;
    let identity = crate::system::process::process_identity::require_core(
        &agent_desktop_core::ProcessIdentity::new(entry.process.pid, instance),
    )?;
    let handle = adapter.resolve_element_strict(entry, deadline)?;
    let target = crate::adapter::ax_element(&handle)?;
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    let window = owning_window(target, identity.pid(), deadline)?;
    let number =
        crate::system::window::window_resolve::ax_window_id_with_deadline(&window, instant)?
            .filter(|number| *number > 0)
            .ok_or_else(missing_owner)?;
    let expected = WindowInfo {
        id: format!("w-{number}"),
        title: String::new(),
        app: String::new(),
        pid: entry.process.pid,
        process_instance: Some(instance.to_string()),
        bounds: None,
        state: Default::default(),
    };
    if !identity.still_matches()? {
        return Err(AdapterError::stale_ref_because(
            "Ref focus window identity changed",
        ));
    }
    Ok(expected)
}

pub(crate) fn owning_window(
    target: &crate::tree::AXElement,
    expected_pid: i32,
    deadline: Deadline,
) -> Result<crate::tree::AXElement, AdapterError> {
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    let window = find_window(
        target.clone(),
        |element| crate::tree::surface_read::string(element, "AXRole", instant),
        |element, attribute| crate::tree::surface_read::element(element, attribute, instant),
    )?
    .ok_or_else(missing_owner)?;
    let pid = crate::tree::properties::ax_ipc::pid(&window, instant).map_err(pid_read_error)?;
    if pid != expected_pid {
        return Err(AdapterError::stale_ref_because(
            "Input target window belongs to another process",
        ));
    }
    Ok(window)
}

fn find_window<T>(
    target: T,
    mut role: impl FnMut(&T) -> Result<Option<String>, AdapterError>,
    mut read: impl FnMut(&T, &str) -> Result<Option<T>, AdapterError>,
) -> Result<Option<T>, AdapterError> {
    let mut current = Some(target);
    for _ in 0..32 {
        let Some(element) = current else {
            return Ok(None);
        };
        if role(&element)?.as_deref() == Some("AXWindow") {
            return Ok(Some(element));
        }
        if let Some(window) = read(&element, "AXWindow")? {
            if role(&window)?.as_deref() != Some("AXWindow") {
                return Err(missing_owner());
            }
            return Ok(Some(window));
        }
        current = read(&element, "AXParent")?;
    }
    Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        "Ref focus window ancestry exceeded its bounded search",
    ))
}

fn pid_read_error(error: i32) -> AdapterError {
    let code = match error {
        accessibility_sys::kAXErrorAPIDisabled => ErrorCode::PermDenied,
        accessibility_sys::kAXErrorCannotComplete => ErrorCode::Timeout,
        accessibility_sys::kAXErrorInvalidUIElement => ErrorCode::StaleRef,
        _ => ErrorCode::ActionFailed,
    };
    AdapterError::new(code, "Could not verify ref focus window process")
        .with_details(serde_json::json!({ "ax_error": error, "physical_delivery_started": false }))
}

fn missing_owner() -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "Headed input requires a verified live owning AXWindow",
    )
    .with_details(serde_json::json!({ "physical_delivery_started": false }))
}

#[cfg(test)]
#[path = "ref_focus_window_tests.rs"]
mod tests;
