use agent_desktop_core::{
    action::{Action, ActionResult},
    adapter::{
        ImageBuffer, NativeHandle, PermissionStatus, PlatformAdapter, ScreenshotTarget, TreeOptions,
        WindowFilter,
    },
    error::AdapterError,
    node::{AccessibilityNode, AppInfo, WindowInfo},
    refs::RefEntry,
};
use rustc_hash::FxHashSet;

pub struct MacOSAdapter;

impl MacOSAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacOSAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PlatformAdapter for MacOSAdapter {
    fn check_permissions(&self) -> PermissionStatus {
        crate::permissions::check()
    }

    fn get_tree(
        &self,
        win: &WindowInfo,
        opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        let mut visited = FxHashSet::default();
        let el = crate::tree::element_for_pid(win.pid);
        crate::tree::build_subtree(&el, 0, opts.max_depth, opts.include_bounds, &mut visited)
            .ok_or_else(|| AdapterError::internal("Empty AX tree for window"))
    }

    fn execute_action(
        &self,
        handle: &NativeHandle,
        action: Action,
    ) -> Result<ActionResult, AdapterError> {
        execute_action_impl(handle, action)
    }

    fn resolve_element(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        resolve_element_impl(entry)
    }

    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        list_windows_impl(filter)
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        list_apps_impl()
    }

    fn focus_window(&self, win: &WindowInfo) -> Result<(), AdapterError> {
        focus_window_impl(win)
    }

    fn launch_app(&self, id: &str, wait: bool) -> Result<WindowInfo, AdapterError> {
        launch_app_impl(id, wait)
    }

    fn close_app(&self, id: &str, force: bool) -> Result<(), AdapterError> {
        close_app_impl(id, force)
    }

    fn screenshot(&self, target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> {
        match target {
            ScreenshotTarget::Window(id) => {
                let window_id = id.parse::<u32>().map_err(|_| {
                    AdapterError::new(
                        agent_desktop_core::error::ErrorCode::InvalidArgs,
                        format!("Invalid window ID: {id}"),
                    )
                })?;
                crate::screenshot::capture_window(window_id)
            }
            ScreenshotTarget::Screen(idx) => crate::screenshot::capture_screen(idx),
            ScreenshotTarget::FullScreen => crate::screenshot::capture_screen(0),
        }
    }

    fn get_clipboard(&self) -> Result<String, AdapterError> {
        crate::clipboard::get()
    }

    fn set_clipboard(&self, text: &str) -> Result<(), AdapterError> {
        crate::clipboard::set(text)
    }

    fn focused_window(&self) -> Result<Option<WindowInfo>, AdapterError> {
        let filter = WindowFilter { focused_only: true, app: None };
        let windows = self.list_windows(&filter)?;
        Ok(windows.into_iter().next())
    }
}

#[cfg(target_os = "macos")]
fn execute_action_impl(handle: &NativeHandle, action: Action) -> Result<ActionResult, AdapterError> {
    use crate::tree::AXElement;
    use core_foundation::base::CFRetain;
    use core_foundation_sys::base::CFTypeRef;

    unsafe { CFRetain(handle.as_raw() as CFTypeRef) };
    let el = AXElement(handle.as_raw() as accessibility_sys::AXUIElementRef);
    let result = crate::actions::perform_action(&el, &action)?;
    std::mem::forget(el);
    Ok(result)
}

#[cfg(not(target_os = "macos"))]
fn execute_action_impl(_handle: &NativeHandle, _action: Action) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("execute_action"))
}

#[cfg(target_os = "macos")]
fn resolve_element_impl(entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    let root = crate::tree::element_for_pid(entry.pid);
    find_element_recursive(&root, entry, 0, 20)
}

#[cfg(target_os = "macos")]
fn find_element_recursive(
    el: &crate::tree::AXElement,
    entry: &RefEntry,
    depth: u8,
    max_depth: u8,
) -> Result<NativeHandle, AdapterError> {
    use accessibility_sys::{
        kAXChildrenAttribute, kAXErrorSuccess, kAXRoleAttribute, kAXTitleAttribute,
        AXUIElementCopyAttributeValue, AXUIElementRef,
    };
    use core_foundation::{
        array::CFArray,
        base::{CFRetain, CFType, CFTypeRef, TCFType},
    };

    let role = crate::tree::copy_string_attr(el, kAXRoleAttribute);
    let normalized = role.as_deref().map(crate::roles::ax_role_to_str).unwrap_or("unknown");

    if normalized == entry.role {
        let name = crate::tree::copy_string_attr(el, kAXTitleAttribute);
        let name_match = match (&entry.name, &name) {
            (Some(en), Some(nn)) => en == nn,
            (None, None) => true,
            _ => false,
        };
        if name_match {
            unsafe { CFRetain(el.0 as CFTypeRef) };
            return Ok(NativeHandle::from_ptr(el.0 as *const _));
        }
    }

    if depth >= max_depth {
        return Err(AdapterError::element_not_found("element"));
    }

    let cf_attr = core_foundation::string::CFString::new(kAXChildrenAttribute);
    let mut children_ref: CFTypeRef = std::ptr::null_mut();
    let err = unsafe {
        AXUIElementCopyAttributeValue(
            el.0,
            cf_attr.as_concrete_TypeRef(),
            &mut children_ref,
        )
    };

    if err != kAXErrorSuccess || children_ref.is_null() {
        return Err(AdapterError::element_not_found("element"));
    }

    let arr = unsafe { CFArray::<CFType>::wrap_under_create_rule(children_ref as _) };
    for item in arr.into_iter() {
        let ptr = item.as_concrete_TypeRef() as AXUIElementRef;
        if ptr.is_null() {
            continue;
        }
        unsafe { CFRetain(ptr as CFTypeRef) };
        let child = crate::tree::AXElement(ptr);
        if let Ok(handle) = find_element_recursive(&child, entry, depth + 1, max_depth) {
            return Ok(handle);
        }
    }

    Err(AdapterError::element_not_found("element"))
}

#[cfg(not(target_os = "macos"))]
fn resolve_element_impl(_entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    Err(AdapterError::not_supported("resolve_element"))
}

fn list_windows_impl(filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let app_filter = filter.app.as_deref().unwrap_or("").to_string();
        let script = r#"
tell application "System Events"
    set winList to {}
    repeat with proc in (processes where background only is false)
        set pName to name of proc as string
        set pPid to (unix id of proc) as string
        set winTitles to name of every window of proc
        repeat with wTitle in winTitles
            set winList to winList & {pName & "|" & pPid & "|" & (wTitle as string)}
        end repeat
    end repeat
    set AppleScript's text item delimiters to linefeed
    return winList as text
end tell
"#;
        let output = Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| AdapterError::internal(format!("osascript failed: {e}")))?;

        let text = String::from_utf8_lossy(&output.stdout);
        let mut windows = Vec::new();
        for (idx, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let mut parts = line.splitn(3, '|');
            let app_name = match parts.next() {
                Some(s) => s.trim().to_string(),
                None => continue,
            };
            let pid: i32 = match parts.next().and_then(|s| s.trim().parse().ok()) {
                Some(p) => p,
                None => continue,
            };
            let title = parts.next().unwrap_or("").trim().to_string();

            if !app_filter.is_empty() && !app_name.eq_ignore_ascii_case(&app_filter) {
                continue;
            }

            windows.push(WindowInfo {
                id: format!("w-{}", idx + 1),
                title,
                app: app_name,
                pid,
                bounds: None,
                is_focused: idx == 0,
            });
        }
        Ok(windows)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = filter;
        Err(AdapterError::not_supported("list_windows"))
    }
}

fn list_apps_impl() -> Result<Vec<AppInfo>, AdapterError> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        let output = Command::new("osascript")
            .arg("-e")
            .arg(
                r#"tell application "System Events"
    set result to ""
    repeat with proc in (processes where background only is false)
        set result to result & (name of proc as string) & "|" & ((unix id of proc) as string) & linefeed
    end repeat
    return result
end tell"#,
            )
            .output()
            .map_err(|e| AdapterError::internal(format!("osascript failed: {e}")))?;

        let text = String::from_utf8_lossy(&output.stdout);
        let apps = text
            .lines()
            .filter(|l| !l.is_empty())
            .filter_map(|line| {
                let mut parts = line.split('|');
                let name = parts.next()?.trim().to_string();
                let pid: i32 = parts.next()?.trim().parse().ok()?;
                Some(AppInfo { name, pid, bundle_id: None })
            })
            .collect();
        Ok(apps)
    }
    #[cfg(not(target_os = "macos"))]
    Err(AdapterError::not_supported("list_apps"))
}

fn focus_window_impl(win: &WindowInfo) -> Result<(), AdapterError> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        Command::new("osascript")
            .arg("-e")
            .arg(format!(r#"tell application "{}" to activate"#, win.app))
            .output()
            .map_err(|e| AdapterError::internal(format!("focus_window failed: {e}")))?;
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = win;
        Err(AdapterError::not_supported("focus_window"))
    }
}

fn launch_app_impl(id: &str, wait: bool) -> Result<WindowInfo, AdapterError> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        use std::time::{Duration, Instant};

        Command::new("open")
            .arg("-a")
            .arg(id)
            .output()
            .map_err(|e| AdapterError::internal(format!("open failed: {e}")))?;

        if wait {
            let start = Instant::now();
            let timeout = Duration::from_secs(10);
            loop {
                std::thread::sleep(Duration::from_millis(200));
                let filter = WindowFilter { focused_only: false, app: Some(id.to_string()) };
                if let Ok(wins) = list_windows_impl(&filter) {
                    if let Some(win) = wins.into_iter().next() {
                        return Ok(win);
                    }
                }
                if start.elapsed() > timeout {
                    break;
                }
            }
        }

        Ok(WindowInfo {
            id: "w-0".into(),
            title: id.to_string(),
            app: id.to_string(),
            pid: 0,
            bounds: None,
            is_focused: true,
        })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (id, wait);
        Err(AdapterError::not_supported("launch_app"))
    }
}

fn close_app_impl(id: &str, force: bool) -> Result<(), AdapterError> {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if force {
            Command::new("pkill")
                .arg("-f")
                .arg(id)
                .output()
                .map_err(|e| AdapterError::internal(format!("pkill failed: {e}")))?;
        } else {
            Command::new("osascript")
                .arg("-e")
                .arg(format!(r#"tell application "{id}" to quit"#))
                .output()
                .map_err(|e| AdapterError::internal(format!("quit failed: {e}")))?;
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (id, force);
        Err(AdapterError::not_supported("close_app"))
    }
}
