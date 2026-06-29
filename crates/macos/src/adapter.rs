use agent_desktop_core::{
    PermissionReport,
    action::{DragParams, MouseEvent, WindowOp},
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::{
        ImageBuffer, LiveElement, NativeHandle, PlatformAdapter, ScreenshotTarget, SnapshotSurface,
        TreeOptions, WindowFilter,
    },
    element_state::ElementState,
    error::AdapterError,
    node::{AccessibilityNode, AppInfo, Rect, SurfaceInfo, WindowInfo},
    notification::{NotificationFilter, NotificationIdentity, NotificationInfo},
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
    fn permission_report(&self) -> PermissionReport {
        crate::system::permissions::report()
    }

    fn request_permissions(&self) -> PermissionReport {
        crate::system::permissions::request_report()
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        false
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
            _ => return Err(AdapterError::not_supported("snapshot surface")),
        };
        let mut visited = FxHashSet::default();
        let context = crate::tree::TreeBuildContext::for_pid(win.pid, opts.include_bounds);
        crate::tree::build_subtree(
            &el,
            0,
            0,
            opts.max_depth,
            &mut visited,
            opts.skeleton,
            &context,
        )
        .ok_or_else(|| AdapterError::internal("Empty AX tree for surface"))
    }

    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        execute_action_impl(handle, request)
    }

    fn resolve_element_strict(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        crate::tree::resolve::resolve_element_impl(entry)
    }

    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        crate::tree::resolve::resolve_element_with_timeout(entry, timeout)
    }

    fn release_handle(&self, handle: &NativeHandle) -> Result<(), AdapterError> {
        let raw = handle.as_raw();
        if raw.is_null() {
            return Ok(());
        }
        unsafe {
            core_foundation::base::CFRelease(raw as core_foundation::base::CFTypeRef);
        }
        Ok(())
    }

    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        crate::system::window_list::list_windows_impl(filter)
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        crate::system::app_list::list_apps_impl()
    }

    fn focus_window(&self, win: &WindowInfo) -> Result<(), AdapterError> {
        crate::system::app_ops::focus_window_impl(win)
    }

    fn focus_app(&self, pid: i32) -> Result<(), AdapterError> {
        crate::system::app_ops::ensure_app_focused(pid)
    }

    fn launch_app(&self, id: &str, timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
        crate::system::app_ops::launch_app_impl(id, timeout_ms)
    }

    fn close_app(&self, id: &str, force: bool) -> Result<(), AdapterError> {
        crate::system::app_ops::close_app_impl(id, force)
    }

    fn is_protected_process(&self, identifier: &str) -> bool {
        crate::system::app_ops::is_protected_process(identifier)
    }

    fn is_blocked_combo(&self, combo: &agent_desktop_core::action::KeyCombo) -> bool {
        crate::input::blocked_combo::is_blocked(combo)
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
    ) -> Result<ActionResult, AdapterError> {
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
            Ok(with_borrowed_ax_element(
                handle,
                crate::tree::copy_value_typed,
            ))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_value"))
    }

    fn get_live_state(&self, handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(Some(with_borrowed_ax_element(
                handle,
                crate::actions::post_state::read_element_state,
            )))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_state"))
    }

    fn get_live_actions(&self, handle: &NativeHandle) -> Result<Option<Vec<String>>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(Some(with_borrowed_ax_element(
                handle,
                crate::actions::post_state::read_live_actions,
            )))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_actions"))
    }

    fn get_live_element(&self, handle: &NativeHandle) -> Result<LiveElement, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(with_borrowed_ax_element(
                handle,
                crate::actions::post_state::read_live_element,
            ))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_element"))
    }

    fn get_element_bounds(&self, handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(with_borrowed_ax_element(handle, crate::tree::read_bounds))
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

    fn key_event(
        &self,
        combo: &agent_desktop_core::action::KeyCombo,
        down: bool,
    ) -> Result<(), AdapterError> {
        crate::input::keyboard::synthesize_key_state(combo, down)
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
        identity: Option<&NotificationIdentity>,
        action_name: &str,
    ) -> Result<ActionResult, AdapterError> {
        crate::notifications::actions::notification_action(index, identity, action_name)
    }

    fn get_subtree(
        &self,
        handle: &NativeHandle,
        opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        with_borrowed_ax_element(handle, |el| {
            let mut ancestors = FxHashSet::default();
            let context = crate::tree::TreeBuildContext::empty(opts.include_bounds);
            crate::tree::build_subtree(
                el,
                0,
                0,
                opts.max_depth,
                &mut ancestors,
                opts.skeleton,
                &context,
            )
            .ok_or_else(|| {
                AdapterError::new(
                    agent_desktop_core::error::ErrorCode::ElementNotFound,
                    "Element no longer exists in accessibility tree",
                )
                .with_suggestion("Run 'snapshot' to refresh refs, then retry.")
            })
        })
    }
}

fn execute_action_impl(
    handle: &NativeHandle,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    with_borrowed_ax_element(handle, |el| crate::actions::perform_action(el, &request))
}

#[cfg(target_os = "macos")]
fn with_borrowed_ax_element<T>(
    handle: &NativeHandle,
    f: impl FnOnce(&crate::tree::AXElement) -> T,
) -> T {
    use std::mem::ManuallyDrop;

    let el = ManuallyDrop::new(crate::tree::AXElement(
        handle.as_raw() as accessibility_sys::AXUIElementRef
    ));
    f(&el)
}
