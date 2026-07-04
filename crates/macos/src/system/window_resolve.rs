use agent_desktop_core::{
    error::{AdapterError, ErrorCode},
    node::WindowInfo,
};

use crate::system::cg_window::WindowRecord;
use crate::tree::{AXElement, copy_ax_array, copy_string_attr, element_for_pid};

#[cfg(target_os = "macos")]
use accessibility_sys::{AXUIElementRef, kAXErrorSuccess, kAXWindowsAttribute};

pub(crate) fn window_element_for_info(win: &WindowInfo) -> Result<AXElement, AdapterError> {
    if win.id.is_empty() {
        return Ok(crate::tree::window_element_for(win.pid, &win.title));
    }
    resolve_window_element_strict(win)
}

pub(crate) fn resolve_window_strict(win: &WindowInfo) -> Result<WindowInfo, AdapterError> {
    let (_, record) = locate_verified_record(win)?;
    Ok(window_info_from_record(win, &record))
}

fn resolve_window_element_strict(win: &WindowInfo) -> Result<AXElement, AdapterError> {
    let (window_number, record) = locate_verified_record(win)?;
    ax_window_element_for_number(win.pid, window_number, record.title.as_deref())
        .ok_or_else(|| window_not_found(&win.id))
}

/// Single owner of the parse-id → find-record → verify-identity chain shared
/// by [`resolve_window_strict`] and [`resolve_window_element_strict`].
fn locate_verified_record(win: &WindowInfo) -> Result<(i64, WindowRecord), AdapterError> {
    let window_number = parse_window_number(&win.id).ok_or_else(|| invalid_window_id(&win.id))?;
    let record = find_window_record(window_number).ok_or_else(|| window_not_found(&win.id))?;
    verify_window_record(win, &record)?;
    Ok((window_number, record))
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

/// Matches the CGWindow-verified `window_number` against the app's live
/// `AXWindow` elements.
///
/// The CGWindow record was already verified (pid + title) by
/// `locate_verified_record`, so a total bridge miss means
/// `_AXUIElementGetWindow` transiently failed (busy/hung app, benign race),
/// not that the window is gone. In that case, fall back to the AX window
/// matching the verified title before reporting `WINDOW_NOT_FOUND` — but only
/// when the title identifies exactly one live `AXWindow`. If zero or two-plus
/// windows share that title, the correct target cannot be determined safely,
/// so this returns `None` and lets the caller surface `WINDOW_NOT_FOUND`
/// rather than risk acting on the wrong same-titled window.
fn ax_window_element_for_number(
    pid: i32,
    window_number: i64,
    fallback_title: Option<&str>,
) -> Option<AXElement> {
    let app = element_for_pid(pid);
    let windows = copy_ax_array(&app, kAXWindowsAttribute)?;
    for window in &windows {
        if copy_string_attr(window, "AXRole").as_deref() != Some("AXWindow") {
            continue;
        }
        if ax_window_id(window) == Some(window_number) {
            return Some(window.clone());
        }
    }
    let title = fallback_title?;
    let mut matches = windows.into_iter().filter(|window| {
        copy_string_attr(window, "AXRole").as_deref() == Some("AXWindow")
            && copy_string_attr(window, "AXTitle").as_deref() == Some(title)
    });
    let single_match = matches.next()?;
    match matches.next() {
        Some(_) => None,
        None => Some(single_match),
    }
}

/// Bridges an accessibility window element to its CoreGraphics window number
/// via the private-but-stable `_AXUIElementGetWindow`, the same call every
/// macOS window manager relies on. The `AXWindowNumber` attribute is not
/// published by AppKit or SwiftUI windows, so this is the only reliable way to
/// match an `AXUIElement` back to the `kCGWindowNumber` that `list-windows`
/// reports.
/// The one owner of the AX-element -> CGWindowID bridge, shared by
/// [`ax_window_element_for_number`], `resolve_roots`'s source-window scoping,
/// and `window_inventory`'s AX-fallback/focus paths, so `AXWindowNumber` (which
/// AppKit/SwiftUI never publish) is not read anywhere.
#[cfg(target_os = "macos")]
pub(crate) fn ax_window_id(window: &AXElement) -> Option<i64> {
    let mut window_id: u32 = 0;
    let result = unsafe { _AXUIElementGetWindow(window.0, &mut window_id) };
    (result == kAXErrorSuccess).then_some(i64::from(window_id))
}

#[cfg(not(target_os = "macos"))]
pub(crate) fn ax_window_id(_window: &AXElement) -> Option<i64> {
    None
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn _AXUIElementGetWindow(element: AXUIElementRef, out: *mut u32) -> i32;
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
