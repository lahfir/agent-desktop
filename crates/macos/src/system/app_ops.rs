use agent_desktop_core::{
    adapter::WindowFilter,
    error::{AdapterError, ErrorCode},
    node::WindowInfo,
};
use std::time::Duration;

#[cfg(target_os = "macos")]
pub fn pid_from_element(el: &crate::tree::AXElement) -> Option<i32> {
    let mut pid: i32 = 0;
    let err = unsafe { accessibility_sys::AXUIElementGetPid(el.0, &mut pid) };
    if err == accessibility_sys::kAXErrorSuccess {
        Some(pid)
    } else {
        None
    }
}

/// Ensures the app is frontmost: a no-op when it already is, otherwise a
/// best-effort raise confirmed by polling. `Ok` therefore means "frontmost
/// ensured", not "a raise happened" — callers surfacing `focused:true` get
/// exactly that ensured semantics.
#[cfg(target_os = "macos")]
pub fn ensure_app_focused(pid: i32) -> Result<(), AdapterError> {
    tracing::debug!("system: ensure_app_focused pid={pid}");
    use accessibility_sys::{AXUIElementSetAttributeValue, kAXErrorSuccess};
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let app_el = crate::tree::element_for_pid(pid);
    if crate::tree::copy_bool_attr(&app_el, "AXFrontmost") == Some(true) {
        return Ok(());
    }
    let frontmost_attr = CFString::new("AXFrontmost");
    let err = unsafe {
        AXUIElementSetAttributeValue(
            app_el.0,
            frontmost_attr.as_concrete_TypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        )
    };
    if err != kAXErrorSuccess {
        return Err(AdapterError::internal(format!(
            "Failed to focus app pid={pid}"
        )));
    }
    wait_until_frontmost(&app_el);
    Ok(())
}

/// Polls `AXFrontmost` until the app actually reports frontmost instead of a
/// fixed settle sleep, so an already-frontmost app costs one read and a slow
/// activation gets the full window. Best-effort: timing out just means the
/// caller proceeds as before the poll existed.
#[cfg(target_os = "macos")]
fn wait_until_frontmost(app_el: &crate::tree::AXElement) {
    use std::time::{Duration, Instant};

    const POLL_INTERVAL: Duration = Duration::from_millis(5);
    const FRONTMOST_DEADLINE: Duration = Duration::from_millis(50);

    let deadline = Instant::now() + FRONTMOST_DEADLINE;
    loop {
        if crate::tree::copy_bool_attr(app_el, "AXFrontmost") == Some(true) {
            return;
        }
        if Instant::now() >= deadline {
            return;
        }
        std::thread::sleep(POLL_INTERVAL);
    }
}

#[cfg(target_os = "macos")]
pub fn focus_window_impl(win: &WindowInfo) -> Result<(), AdapterError> {
    tracing::debug!(
        "system: focus_window app={:?} title={:?}",
        win.app,
        win.title
    );
    ensure_app_focused(win.pid)?;
    let main_win = crate::tree::window_element_for(win.pid, &win.title);
    crate::system::window_ops::raise_window(&main_win);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn focus_window_impl(_win: &WindowInfo) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("focus_window"))
}

#[cfg(target_os = "macos")]
pub fn launch_app_impl(id: &str, timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
    tracing::debug!("system: launch app={id:?} timeout={timeout_ms}ms");
    use crate::system::window_list::list_windows_impl;
    use std::process::Command;
    use std::time::{Duration, Instant};

    const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

    if id.contains("..") || id.starts_with('/') {
        return Err(AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("Invalid app identifier: '{id}'"),
        )
        .with_suggestion("Use an app name like 'Safari' or bundle ID like 'com.apple.Safari'."));
    }

    let filter = WindowFilter {
        focused_only: false,
        app: Some(id.to_string()),
    };
    if let Ok(wins) = list_windows_impl(&filter) {
        if let Some(win) = wins.into_iter().next() {
            return Ok(win);
        }
    }

    let mut command = Command::new("/usr/bin/open");
    command.args(open_app_args(id));
    crate::system::process::run_with_timeout(&mut command, "open", OPEN_TIMEOUT)?;

    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut poll_interval = Duration::from_millis(100);
    let max_interval = Duration::from_millis(500);

    loop {
        std::thread::sleep(poll_interval);
        let filter = WindowFilter {
            focused_only: false,
            app: Some(id.to_string()),
        };
        if let Ok(wins) = list_windows_impl(&filter) {
            if let Some(win) = wins.into_iter().next() {
                return Ok(win);
            }
        }
        if start.elapsed() > timeout {
            break;
        }
        poll_interval = (poll_interval * 3 / 2).min(max_interval);
    }

    Err(AdapterError::new(
        agent_desktop_core::error::ErrorCode::AppNotFound,
        format!("App '{id}' launched but no window appeared within {timeout_ms} ms"),
    )
    .with_suggestion("The app may take longer to start, or it may not create a visible window"))
}

#[cfg(target_os = "macos")]
fn open_app_args(id: &str) -> [&str; 3] {
    ["-g", "-a", id]
}

#[cfg(not(target_os = "macos"))]
pub fn launch_app_impl(_id: &str, _timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
    Err(AdapterError::not_supported("launch_app"))
}

/// Processes whose termination would break the macOS session: the window
/// server, login session, launchd, the Dock, and Finder. Matched as an
/// exact lowercase name or an exact dot-separated bundle-id component, so
/// display names (`Dock`) and bundle ids (`com.apple.dock`) both resolve
/// while lookalikes (`Docker`, `FinderSync`) stay closable — a substring
/// match would permanently block them. Windows and Linux adapters define
/// their own equivalents (`csrss.exe`/`winlogon.exe`, `gnome-shell`/`Xorg`).
const PROTECTED_PROCESSES: &[&str] = &["loginwindow", "windowserver", "dock", "launchd", "finder"];

pub fn is_protected_process(identifier: &str) -> bool {
    let lower = identifier.to_lowercase();
    PROTECTED_PROCESSES
        .iter()
        .any(|p| lower == *p || lower.split('.').any(|component| component == *p))
}

fn ensure_not_protected(id: &str) -> Result<(), AdapterError> {
    if is_protected_process(id) {
        return Err(AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("'{id}' is a protected system process and cannot be closed"),
        )
        .with_suggestion(
            "Target a regular application; session-critical processes (loginwindow, WindowServer, Dock, Finder, launchd) are never closed.",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "app_ops_tests.rs"]
mod tests;

/// Closes an app after the protected-process guard. The guard runs here —
/// inside the adapter — so every consumer (CLI, FFI, future MCP) refuses
/// session-critical processes identically; the CLI command's own preflight
/// is an earlier check against the same predicate, not the enforcement
/// point. The error mirrors the CLI contract exactly (code and message).
#[cfg(target_os = "macos")]
pub fn close_app_impl(id: &str, force: bool) -> Result<(), AdapterError> {
    ensure_not_protected(id)?;
    tracing::debug!("system: close app={id:?} force={force}");
    use std::process::Command;

    const QUIT_TIMEOUT: Duration = Duration::from_secs(3);

    if force {
        let pids = crate::system::app_list::pids_for_app_name(id);
        if pids.is_empty() {
            return Err(AdapterError::new(
                ErrorCode::AppNotFound,
                format!("App '{id}' was not running or could not be matched for force close"),
            )
            .with_suggestion("Use 'list-apps' to verify the running app name before retrying."));
        }
        crate::system::force_close::terminate_app(id, &pids, QUIT_TIMEOUT)?;
    } else {
        let pid = crate::system::key_dispatch::find_pid_by_name(id)?;
        let app_ax = crate::tree::element_for_pid(pid);
        let closed = try_quit_via_menu_bar(&app_ax);
        if !closed {
            if id
                .chars()
                .any(|c| !c.is_alphanumeric() && !matches!(c, ' ' | '-' | '.' | '_'))
            {
                return Err(AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    format!("Invalid app name '{id}'"),
                )
                .with_suggestion("App name should only contain letters, numbers, spaces, hyphens, dots, or underscores."));
            }
            let script = format!(
                r#"tell application "System Events"
    set theProc to first process whose name is "{id}"
    tell theProc to quit
end tell"#
            );
            let mut command = Command::new("/usr/bin/osascript");
            command.arg("-e").arg(script);
            let output =
                crate::system::process::run_with_timeout(&mut command, "osascript", QUIT_TIMEOUT)?;
            if !output.status.success() {
                return Err(AdapterError::new(
                    ErrorCode::ActionFailed,
                    format!("Failed to request graceful quit for app '{id}'"),
                )
                .with_platform_detail(String::from_utf8_lossy(&output.stderr).trim().to_string())
                .with_suggestion(
                    "Use 'list-apps' to verify the app name, or retry with --force.",
                ));
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn try_quit_via_menu_bar(app_el: &crate::tree::AXElement) -> bool {
    use accessibility_sys::{AXUIElementPerformAction, kAXErrorSuccess};
    use core_foundation::{base::TCFType, string::CFString};

    let Some(menu_bar) = crate::tree::copy_element_attr(app_el, "AXMenuBar") else {
        return false;
    };
    let Some(bar_items) = crate::tree::copy_ax_array(&menu_bar, "AXChildren") else {
        return false;
    };
    for bar_item in bar_items.iter().skip(1) {
        let Some(menus) = crate::tree::copy_ax_array(bar_item, "AXChildren") else {
            continue;
        };
        for menu in &menus {
            let Some(items) = crate::tree::copy_ax_array(menu, "AXChildren") else {
                continue;
            };
            for item in &items {
                let Some(t) = crate::tree::copy_string_attr(item, "AXTitle") else {
                    continue;
                };
                if t.starts_with("Quit") {
                    let press = CFString::new("AXPress");
                    let err =
                        unsafe { AXUIElementPerformAction(item.0, press.as_concrete_TypeRef()) };
                    return err == kAXErrorSuccess;
                }
            }
        }
    }
    false
}

#[cfg(not(target_os = "macos"))]
pub fn close_app_impl(_id: &str, _force: bool) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("close_app"))
}
