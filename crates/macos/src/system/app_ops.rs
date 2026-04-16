use agent_desktop_core::{
    adapter::WindowFilter,
    error::AdapterError,
    node::{AppInfo, WindowInfo},
};

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

#[cfg(target_os = "macos")]
pub fn ensure_app_focused(pid: i32) -> Result<(), AdapterError> {
    tracing::debug!("system: ensure_app_focused pid={pid}");
    use accessibility_sys::{kAXErrorSuccess, AXUIElementSetAttributeValue};
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let app_el = crate::tree::element_for_pid(pid);
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
    std::thread::sleep(std::time::Duration::from_millis(50));
    Ok(())
}

#[cfg(target_os = "macos")]
pub fn focus_window_impl(win: &WindowInfo) -> Result<(), AdapterError> {
    tracing::debug!(
        "system: focus_window app={:?} title={:?}",
        win.app,
        win.title
    );
    use accessibility_sys::{
        kAXErrorSuccess, AXUIElementCreateApplication, AXUIElementPerformAction,
        AXUIElementSetAttributeValue,
    };
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let app_el = crate::tree::AXElement(unsafe { AXUIElementCreateApplication(win.pid) });
    if app_el.0.is_null() {
        return Err(AdapterError::internal("Failed to create AX app element"));
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
    tracing::debug!("system: launch app={id:?} timeout={timeout_ms}ms");
    use crate::adapter::list_windows_impl;
    use std::process::Command;
    use std::time::{Duration, Instant};

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
    tracing::debug!("system: close app={id:?} force={force}");
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

pub fn list_apps_impl() -> Result<Vec<AppInfo>, AdapterError> {
    #[cfg(target_os = "macos")]
    {
        use core_foundation::base::{CFType, TCFType};
        use core_foundation::number::CFNumber;
        use core_foundation::string::CFString;
        use core_foundation_sys::dictionary::CFDictionaryGetValue;
        use core_graphics::display::CGDisplay;
        use core_graphics::window::{
            kCGWindowLayer, kCGWindowListOptionOnScreenOnly, kCGWindowOwnerName, kCGWindowOwnerPID,
        };

        let arr = match CGDisplay::window_list_info(kCGWindowListOptionOnScreenOnly, None) {
            Some(a) => a,
            None => return Ok(vec![]),
        };

        let mut seen_pids = std::collections::HashSet::new();
        let mut apps = Vec::new();

        for raw in arr.get_all_values() {
            if raw.is_null() {
                continue;
            }

            let layer = unsafe {
                let v = CFDictionaryGetValue(raw as _, kCGWindowLayer as _);
                if v.is_null() {
                    continue;
                }
                CFType::wrap_under_get_rule(v as _)
                    .downcast::<CFNumber>()
                    .and_then(|n| n.to_i64())
                    .unwrap_or(99)
            };
            if layer != 0 {
                continue;
            }

            let pid = unsafe {
                let v = CFDictionaryGetValue(raw as _, kCGWindowOwnerPID as _);
                if v.is_null() {
                    continue;
                }
                CFType::wrap_under_get_rule(v as _)
                    .downcast::<CFNumber>()
                    .and_then(|n| n.to_i64())
                    .unwrap_or(0) as i32
            };
            if !seen_pids.insert(pid) {
                continue;
            }

            let name = unsafe {
                let v = CFDictionaryGetValue(raw as _, kCGWindowOwnerName as _);
                if v.is_null() {
                    continue;
                }
                CFType::wrap_under_get_rule(v as _)
                    .downcast::<CFString>()
                    .map(|s| s.to_string())
            };

            if let Some(n) = name {
                apps.push(AppInfo {
                    name: n,
                    pid,
                    bundle_id: None,
                });
            }
        }
        Ok(apps)
    }
    #[cfg(not(target_os = "macos"))]
    Err(AdapterError::not_supported("list_apps"))
}
