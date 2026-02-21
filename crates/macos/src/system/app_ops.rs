use agent_desktop_core::{adapter::WindowFilter, error::AdapterError, node::WindowInfo};

#[cfg(target_os = "macos")]
pub fn focus_window_impl(win: &WindowInfo) -> Result<(), AdapterError> {
    use accessibility_sys::{
        kAXErrorSuccess, AXUIElementCreateApplication, AXUIElementPerformAction,
        AXUIElementSetAttributeValue,
    };
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let app_el = unsafe { AXUIElementCreateApplication(win.pid) };
    if app_el.is_null() {
        return Err(AdapterError::internal("Failed to create AX app element"));
    }

    let frontmost_attr = CFString::new("AXFrontmost");
    let err = unsafe {
        AXUIElementSetAttributeValue(
            app_el,
            frontmost_attr.as_concrete_TypeRef(),
            CFBoolean::true_value().as_CFTypeRef(),
        )
    };
    if err != kAXErrorSuccess {
        return Err(AdapterError::internal(format!(
            "Failed to set AXFrontmost (err={err})"
        )));
    }

    let main_win = crate::tree::window_element_for(win.pid, &win.title);
    let raise_action = CFString::new("AXRaise");
    let raise_err =
        unsafe { AXUIElementPerformAction(main_win.0, raise_action.as_concrete_TypeRef()) };
    if raise_err != kAXErrorSuccess {
        let main_attr = CFString::new("AXMain");
        unsafe {
            AXUIElementSetAttributeValue(
                main_win.0,
                main_attr.as_concrete_TypeRef(),
                CFBoolean::true_value().as_CFTypeRef(),
            )
        };
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn focus_window_impl(_win: &WindowInfo) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("focus_window"))
}

#[cfg(target_os = "macos")]
pub fn launch_app_impl(id: &str, timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
    use crate::adapter::list_windows_impl;
    use std::process::Command;
    use std::time::{Duration, Instant};

    if id.contains("..") || id.starts_with('/') {
        return Err(AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("Invalid app identifier: '{id}'"),
        ));
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

    Command::new("open")
        .arg("-a")
        .arg(id)
        .output()
        .map_err(|e| AdapterError::internal(format!("open failed: {e}")))?;

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

#[cfg(not(target_os = "macos"))]
pub fn launch_app_impl(_id: &str, _timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
    Err(AdapterError::not_supported("launch_app"))
}

#[cfg(target_os = "macos")]
pub fn close_app_impl(id: &str, force: bool) -> Result<(), AdapterError> {
    use std::process::Command;
    if force {
        Command::new("pkill")
            .arg("-x")
            .arg(id)
            .output()
            .map_err(|e| AdapterError::internal(format!("pkill failed: {e}")))?;
    } else {
        let pid = crate::system::key_dispatch::find_pid_by_name(id)?;
        let app_ax = crate::tree::element_for_pid(pid);
        let closed = try_quit_via_menu_bar(&app_ax);
        if !closed {
            let safe_name = id.replace('"', "");
            let script = format!(
                r#"tell application "System Events"
    set theProc to first process whose name is "{safe_name}"
    tell theProc to quit
end tell"#
            );
            Command::new("osascript")
                .arg("-e")
                .arg(script)
                .output()
                .map_err(|e| AdapterError::internal(format!("quit failed: {e}")))?;
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn try_quit_via_menu_bar(app_el: &crate::tree::AXElement) -> bool {
    use accessibility_sys::{kAXErrorSuccess, AXUIElementPerformAction};
    use core_foundation::{base::TCFType, string::CFString};

    let Some(menu_bar) = crate::tree::copy_element_attr(app_el, "AXMenuBar") else {
        return false;
    };
    let Some(bar_items) = crate::tree::copy_ax_array(&menu_bar, "AXChildren") else {
        return false;
    };
    if let Some(app_menu) = bar_items.first() {
        if let Some(menus) = crate::tree::copy_ax_array(app_menu, "AXChildren") {
            for menu in &menus {
                if let Some(items) = crate::tree::copy_ax_array(menu, "AXChildren") {
                    for item in &items {
                        let title = crate::tree::copy_string_attr(item, "AXTitle");
                        if let Some(t) = &title {
                            if t.contains("Quit") || t.contains("quit") {
                                let press = CFString::new("AXPress");
                                let err = unsafe {
                                    AXUIElementPerformAction(item.0, press.as_concrete_TypeRef())
                                };
                                return err == kAXErrorSuccess;
                            }
                        }
                    }
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
