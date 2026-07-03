use agent_desktop_core::{
    error::{AdapterError, ErrorCode},
    node::WindowInfo,
};

use crate::system::cg_window::WindowRecord;
use crate::tree::{AXElement, copy_ax_array, copy_i64_attr, copy_string_attr, element_for_pid};

#[cfg(target_os = "macos")]
use accessibility_sys::kAXWindowsAttribute;

pub(crate) fn window_element_for_info(win: &WindowInfo) -> Result<AXElement, AdapterError> {
    if win.id.is_empty() {
        return Ok(crate::tree::window_element_for(win.pid, &win.title));
    }
    resolve_window_element_strict(win)
}

pub(crate) fn resolve_window_strict(win: &WindowInfo) -> Result<WindowInfo, AdapterError> {
    let window_number = parse_window_number(&win.id).ok_or_else(|| invalid_window_id(&win.id))?;
    let record = find_window_record(window_number).ok_or_else(|| window_not_found(&win.id))?;
    verify_window_record(win, &record)?;
    Ok(window_info_from_record(win, &record))
}

fn resolve_window_element_strict(win: &WindowInfo) -> Result<AXElement, AdapterError> {
    let window_number = parse_window_number(&win.id).ok_or_else(|| invalid_window_id(&win.id))?;
    verify_window_record(
        win,
        &find_window_record(window_number).ok_or_else(|| window_not_found(&win.id))?,
    )?;
    ax_window_element_for_number(win.pid, window_number).ok_or_else(|| window_not_found(&win.id))
}

pub(crate) fn parse_window_number(id: &str) -> Option<i64> {
    id.strip_prefix('w')?.strip_prefix('-')?.parse().ok()
}

fn find_window_record(window_number: i64) -> Option<WindowRecord> {
    crate::system::cg_window::visible_window_records()
        .into_iter()
        .find(|record| record.window_number == window_number)
}

fn verify_window_record(win: &WindowInfo, record: &WindowRecord) -> Result<(), AdapterError> {
    if record.pid != win.pid {
        return Err(window_identity_mismatch(&win.id));
    }
    if !win.title.is_empty() {
        let record_title = record.title.as_deref().unwrap_or(record.app_name.as_str());
        if record_title != win.title {
            return Err(window_identity_mismatch(&win.id));
        }
    }
    Ok(())
}

fn window_info_from_record(win: &WindowInfo, record: &WindowRecord) -> WindowInfo {
    WindowInfo {
        id: win.id.clone(),
        title: record
            .title
            .clone()
            .unwrap_or_else(|| record.app_name.clone()),
        app: record.app_name.clone(),
        pid: record.pid,
        bounds: None,
        is_focused: win.is_focused,
    }
}

fn ax_window_element_for_number(pid: i32, window_number: i64) -> Option<AXElement> {
    let app = element_for_pid(pid);
    let windows = copy_ax_array(&app, kAXWindowsAttribute)?;
    for window in &windows {
        if copy_string_attr(window, "AXRole").as_deref() != Some("AXWindow") {
            continue;
        }
        if copy_i64_attr(window, "AXWindowNumber") == Some(window_number) {
            return Some(window.clone());
        }
    }
    None
}

fn invalid_window_id(id: &str) -> AdapterError {
    AdapterError::new(ErrorCode::InvalidArgs, format!("Invalid window id: '{id}'"))
        .with_suggestion("Window ids come from 'list-windows' (format w-<number>).")
}

fn window_not_found(id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        format!("Window '{id}' not found"),
    )
    .with_suggestion("Run 'list-windows' to see available windows and their IDs.")
}

fn window_identity_mismatch(id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::WindowNotFound,
        format!("Window '{id}' identity mismatch"),
    )
    .with_suggestion("Run 'list-windows' to refresh window IDs, then retry.")
}

#[cfg(test)]
#[path = "window_resolve_tests.rs"]
mod tests;
