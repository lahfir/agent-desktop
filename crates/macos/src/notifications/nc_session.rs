use agent_desktop_core::error::AdapterError;

pub struct NcSession {
    was_already_open: bool,
}

impl NcSession {
    pub fn open() -> Result<Self, AdapterError> {
        let was_already_open = is_nc_open();
        if !was_already_open {
            open_nc()?;
            wait_for_nc_ready()?;
        }
        Ok(Self { was_already_open })
    }

    pub fn close(self) -> Result<(), AdapterError> {
        if !self.was_already_open {
            close_nc()?;
        }
        std::mem::forget(self);
        Ok(())
    }
}

impl Drop for NcSession {
    fn drop(&mut self) {
        if !self.was_already_open {
            if let Err(e) = close_nc() {
                tracing::warn!("Failed to close NC in Drop: {e}");
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn nc_pid() -> Option<i32> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_foundation_sys::dictionary::CFDictionaryGetValue;
    use core_graphics::display::CGDisplay;
    use core_graphics::window::{
        kCGWindowListOptionOnScreenOnly, kCGWindowOwnerName, kCGWindowOwnerPID,
    };

    let arr = CGDisplay::window_list_info(kCGWindowListOptionOnScreenOnly, None)?;
    for raw in arr.get_all_values() {
        if raw.is_null() {
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
        if name.as_deref() == Some("NotificationCenter") {
            let pid = unsafe {
                let v = CFDictionaryGetValue(raw as _, kCGWindowOwnerPID as _);
                if v.is_null() {
                    return None;
                }
                CFType::wrap_under_get_rule(v as _)
                    .downcast::<CFNumber>()
                    .and_then(|n| n.to_i64())
                    .map(|p| p as i32)
            };
            return pid;
        }
    }
    None
}

#[cfg(target_os = "macos")]
fn is_nc_open() -> bool {
    use crate::tree::{copy_ax_array, element_for_pid};

    let pid = match nc_pid() {
        Some(p) => p,
        None => return false,
    };
    let app = element_for_pid(pid);
    let windows = copy_ax_array(&app, "AXWindows").unwrap_or_default();
    !windows.is_empty()
}

#[cfg(not(target_os = "macos"))]
fn is_nc_open() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn open_nc() -> Result<(), AdapterError> {
    use crate::tree::{copy_ax_array, copy_string_attr, element_for_pid};
    use accessibility_sys::kAXRoleAttribute;

    let sui_pid = find_system_ui_server_pid()?;
    let sui_app = element_for_pid(sui_pid);
    let menubar = copy_ax_array(&sui_app, "AXMenuBar")
        .or_else(|| copy_ax_array(&sui_app, "AXExtrasMenuBar"))
        .ok_or_else(|| AdapterError::internal("SystemUIServer: no menu bar found"))?;

    let mut clock_item = None;
    for bar_items in [
        copy_ax_array(&sui_app, "AXExtrasMenuBar").unwrap_or_default(),
        menubar,
    ] {
        for item in &bar_items {
            let role = copy_string_attr(item, kAXRoleAttribute);
            if role.as_deref() != Some("AXMenuBarItem") && role.as_deref() != Some("AXMenuExtra") {
                continue;
            }
            let desc = copy_string_attr(item, "AXDescription");
            let title = copy_string_attr(item, "AXTitle");
            let is_clock = desc
                .as_deref()
                .map(|d| d.contains("Clock") || d.contains("clock") || d.contains("Date"))
                .unwrap_or(false)
                || title
                    .as_deref()
                    .map(|t| t.contains("Clock") || t.contains("clock") || t.contains("Date"))
                    .unwrap_or(false);
            if is_clock {
                clock_item = Some(item.clone());
                break;
            }
        }
        if clock_item.is_some() {
            break;
        }
    }

    let clock = clock_item
        .ok_or_else(|| AdapterError::internal("SystemUIServer: clock menu bar item not found"))?;

    if !crate::actions::ax_helpers::try_ax_action(&clock, "AXPress") {
        return Err(AdapterError::internal(
            "Failed to press clock menu bar item to open Notification Center",
        ));
    }

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn open_nc() -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("open_nc"))
}

#[cfg(target_os = "macos")]
fn close_nc() -> Result<(), AdapterError> {
    use crate::input::keyboard;
    use agent_desktop_core::action::KeyCombo;

    let combo = KeyCombo {
        key: "escape".into(),
        modifiers: vec![],
    };
    keyboard::synthesize_key(&combo)?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn close_nc() -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("close_nc"))
}

#[cfg(target_os = "macos")]
fn wait_for_nc_ready() -> Result<(), AdapterError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let poll = std::time::Duration::from_millis(50);

    loop {
        if is_nc_open() {
            return Ok(());
        }
        if std::time::Instant::now() > deadline {
            return Err(AdapterError::timeout(
                "Notification Center did not open within 2 seconds",
            ));
        }
        std::thread::sleep(poll);
    }
}

#[cfg(not(target_os = "macos"))]
fn wait_for_nc_ready() -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("wait_for_nc_ready"))
}

#[cfg(target_os = "macos")]
fn find_system_ui_server_pid() -> Result<i32, AdapterError> {
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::number::CFNumber;
    use core_foundation::string::CFString;
    use core_foundation_sys::dictionary::CFDictionaryGetValue;
    use core_graphics::display::CGDisplay;
    use core_graphics::window::{
        kCGWindowListOptionOnScreenOnly, kCGWindowOwnerName, kCGWindowOwnerPID,
    };

    let arr = CGDisplay::window_list_info(kCGWindowListOptionOnScreenOnly, None)
        .ok_or_else(|| AdapterError::internal("Failed to get window list"))?;

    for raw in arr.get_all_values() {
        if raw.is_null() {
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
        if name.as_deref() == Some("SystemUIServer") {
            let pid = unsafe {
                let v = CFDictionaryGetValue(raw as _, kCGWindowOwnerPID as _);
                if v.is_null() {
                    return Err(AdapterError::internal("SystemUIServer: no PID"));
                }
                CFType::wrap_under_get_rule(v as _)
                    .downcast::<CFNumber>()
                    .and_then(|n| n.to_i64())
                    .map(|p| p as i32)
                    .ok_or_else(|| AdapterError::internal("SystemUIServer: bad PID"))?
            };
            return Ok(pid);
        }
    }
    Err(AdapterError::internal("SystemUIServer process not found"))
}
