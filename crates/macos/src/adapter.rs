use agent_desktop_core::{
    action::{Action, ActionResult, DragParams, MouseEvent, WindowOp},
    adapter::{
        ImageBuffer, NativeHandle, PermissionStatus, PlatformAdapter, ScreenshotTarget,
        SnapshotSurface, TreeOptions, WindowFilter,
    },
    error::AdapterError,
    node::{AccessibilityNode, AppInfo, Rect, SurfaceInfo, WindowInfo},
    notification::{NotificationFilter, NotificationInfo},
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
        crate::system::permissions::check()
    }

    fn get_tree(
        &self,
        win: &WindowInfo,
        opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        let el = match opts.surface {
            SnapshotSurface::Window => crate::tree::window_element_for(win.pid, &win.title),
            SnapshotSurface::Focused => crate::tree::surfaces::focused_surface_for_pid(win.pid)
                .ok_or_else(|| AdapterError::internal("No focused surface found"))?,
            SnapshotSurface::Menu => crate::tree::surfaces::menu_element_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No open context menu"))?,
            SnapshotSurface::Menubar => crate::tree::surfaces::menubar_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No menu bar found"))?,
            SnapshotSurface::Sheet => crate::tree::surfaces::sheet_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No open sheet"))?,
            SnapshotSurface::Popover => crate::tree::surfaces::popover_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No visible popover"))?,
            SnapshotSurface::Alert => crate::tree::surfaces::alert_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No open alert or dialog"))?,
        };
        let mut visited = FxHashSet::default();
        crate::tree::build_subtree(&el, 0, opts.max_depth, opts.include_bounds, &mut visited)
            .ok_or_else(|| AdapterError::internal("Empty AX tree for surface"))
    }

    fn execute_action(
        &self,
        handle: &NativeHandle,
        action: Action,
    ) -> Result<ActionResult, AdapterError> {
        execute_action_impl(handle, action)
    }

    fn resolve_element(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        crate::tree::resolve::resolve_element_impl(entry)
    }

    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        list_windows_impl(filter)
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        list_apps_impl()
    }

    fn focus_window(&self, win: &WindowInfo) -> Result<(), AdapterError> {
        crate::system::app_ops::focus_window_impl(win)
    }

    fn launch_app(&self, id: &str, timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
        crate::system::app_ops::launch_app_impl(id, timeout_ms)
    }

    fn close_app(&self, id: &str, force: bool) -> Result<(), AdapterError> {
        crate::system::app_ops::close_app_impl(id, force)
    }

    fn screenshot(&self, target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> {
        match target {
            ScreenshotTarget::Window(pid) => crate::system::screenshot::capture_app(pid),
            ScreenshotTarget::Screen(idx) => crate::system::screenshot::capture_screen(idx),
            ScreenshotTarget::FullScreen => crate::system::screenshot::capture_screen(0),
        }
    }

    fn get_clipboard(&self) -> Result<String, AdapterError> {
        crate::input::clipboard::get()
    }

    fn set_clipboard(&self, text: &str) -> Result<(), AdapterError> {
        crate::input::clipboard::set(text)
    }

    fn press_key_for_app(
        &self,
        app_name: &str,
        combo: &agent_desktop_core::action::KeyCombo,
    ) -> Result<agent_desktop_core::action::ActionResult, AdapterError> {
        crate::system::key_dispatch::press_for_app_impl(app_name, combo)
    }

    fn wait_for_menu(&self, pid: i32, open: bool, timeout_ms: u64) -> Result<(), AdapterError> {
        crate::system::wait::wait_for_menu(pid, open, timeout_ms)
    }

    fn list_surfaces(&self, pid: i32) -> Result<Vec<SurfaceInfo>, AdapterError> {
        Ok(crate::tree::surfaces::list_surfaces_for_pid(pid))
    }

    fn focused_window(&self) -> Result<Option<WindowInfo>, AdapterError> {
        let filter = WindowFilter {
            focused_only: true,
            app: None,
        };
        let windows = self.list_windows(&filter)?;
        Ok(windows.into_iter().next())
    }

    fn get_live_value(&self, handle: &NativeHandle) -> Result<Option<String>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            use crate::tree::AXElement;
            use accessibility_sys::kAXValueAttribute;
            use std::mem::ManuallyDrop;
            let el = ManuallyDrop::new(AXElement(
                handle.as_raw() as accessibility_sys::AXUIElementRef
            ));
            Ok(crate::tree::copy_string_attr(&el, kAXValueAttribute))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_value"))
    }

    fn get_element_bounds(&self, handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            use crate::tree::AXElement;
            use std::mem::ManuallyDrop;
            let el = ManuallyDrop::new(AXElement(
                handle.as_raw() as accessibility_sys::AXUIElementRef
            ));
            Ok(crate::tree::read_bounds(&el))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = handle;
            Err(AdapterError::not_supported("get_element_bounds"))
        }
    }

    fn window_op(&self, win: &WindowInfo, op: WindowOp) -> Result<(), AdapterError> {
        crate::system::window_ops::execute(win, op)
    }

    fn mouse_event(&self, event: MouseEvent) -> Result<(), AdapterError> {
        crate::input::mouse::synthesize_mouse(event)
    }

    fn drag(&self, params: DragParams) -> Result<(), AdapterError> {
        crate::input::mouse::synthesize_drag(params)
    }

    fn clear_clipboard(&self) -> Result<(), AdapterError> {
        crate::input::clipboard::clear()
    }

    fn list_notifications(
        &self,
        filter: &NotificationFilter,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        crate::notifications::list::list_notifications(filter)
    }

    fn dismiss_notification(
        &self,
        index: usize,
        app_filter: Option<&str>,
    ) -> Result<NotificationInfo, AdapterError> {
        crate::notifications::actions::dismiss_notification(index, app_filter)
    }

    fn dismiss_all_notifications(
        &self,
        app_filter: Option<&str>,
    ) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
        crate::notifications::actions::dismiss_all(app_filter)
    }

    fn notification_action(
        &self,
        index: usize,
        action_name: &str,
    ) -> Result<ActionResult, AdapterError> {
        crate::notifications::actions::notification_action(index, action_name)
    }
}

#[cfg(target_os = "macos")]
fn execute_action_impl(
    handle: &NativeHandle,
    action: Action,
) -> Result<ActionResult, AdapterError> {
    use crate::tree::AXElement;
    use std::mem::ManuallyDrop;

    let el = ManuallyDrop::new(AXElement(
        handle.as_raw() as accessibility_sys::AXUIElementRef
    ));
    crate::actions::perform_action(&el, &action)
}

#[cfg(not(target_os = "macos"))]
fn execute_action_impl(
    _handle: &NativeHandle,
    _action: Action,
) -> Result<ActionResult, AdapterError> {
    Err(AdapterError::not_supported("execute_action"))
}

pub(crate) fn list_windows_impl(filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
    #[cfg(target_os = "macos")]
    {
        use core_foundation::base::{CFType, TCFType};
        use core_foundation::number::CFNumber;
        use core_foundation::string::CFString;
        use core_foundation_sys::dictionary::CFDictionaryGetValue;
        use core_graphics::display::CGDisplay;
        use core_graphics::window::{
            kCGWindowLayer, kCGWindowListOptionOnScreenOnly, kCGWindowName, kCGWindowOwnerName,
            kCGWindowOwnerPID,
        };
        use rustc_hash::FxHasher;
        use std::ffi::c_void;
        use std::hash::{Hash, Hasher};

        unsafe fn dict_string(dict: *const c_void, key: *const c_void) -> Option<String> {
            let val = CFDictionaryGetValue(dict as _, key);
            if val.is_null() {
                return None;
            }
            CFType::wrap_under_get_rule(val as _)
                .downcast::<CFString>()
                .map(|s| s.to_string())
        }

        unsafe fn dict_i64(dict: *const c_void, key: *const c_void) -> Option<i64> {
            let val = CFDictionaryGetValue(dict as _, key);
            if val.is_null() {
                return None;
            }
            CFType::wrap_under_get_rule(val as _)
                .downcast::<CFNumber>()
                .and_then(|n| n.to_i64())
        }

        let arr = match CGDisplay::window_list_info(kCGWindowListOptionOnScreenOnly, None) {
            Some(a) => a,
            None => return Ok(vec![]),
        };

        let app_filter = filter.app.as_deref().unwrap_or("").to_lowercase();
        let mut windows = Vec::new();

        for raw in arr.get_all_values() {
            if raw.is_null() {
                continue;
            }
            let layer = unsafe { dict_i64(raw, kCGWindowLayer as _) }.unwrap_or(99);
            if layer != 0 {
                continue;
            }

            let app_name = match unsafe { dict_string(raw, kCGWindowOwnerName as _) } {
                Some(n) if !n.is_empty() => n,
                _ => continue,
            };
            if !app_filter.is_empty() && !app_name.to_lowercase().contains(&app_filter) {
                continue;
            }

            let title = match unsafe { dict_string(raw, kCGWindowName as _) } {
                Some(t) if !t.is_empty() => t,
                _ => app_name.clone(),
            };

            let pid = unsafe { dict_i64(raw, kCGWindowOwnerPID as _) }.unwrap_or(0) as i32;
            let mut h = FxHasher::default();
            pid.hash(&mut h);
            title.hash(&mut h);
            let id = format!("w-{:x}", h.finish() & 0xFFFFFF);

            windows.push(WindowInfo {
                id,
                title,
                app: app_name,
                pid,
                bounds: None,
                is_focused: windows.is_empty(),
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
