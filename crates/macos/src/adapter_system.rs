use agent_desktop_core::{
    PermissionReport,
    action::WindowOp,
    action_result::ActionResult,
    adapter::{
        ImageBuffer, ObservationOps, ScreenshotTarget, SnapshotSurface, SystemOps, WindowFilter,
    },
    error::AdapterError,
    node::WindowInfo,
    notification::{NotificationFilter, NotificationIdentity, NotificationInfo},
};

use crate::adapter::MacOSAdapter;

impl SystemOps for MacOSAdapter {
    fn permission_report(&self) -> PermissionReport {
        crate::system::permissions::report()
    }

    fn request_permissions(&self) -> PermissionReport {
        crate::system::permissions::request_report()
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        false
    }

    fn focus_window(&self, win: &WindowInfo) -> Result<(), AdapterError> {
        crate::system::app_ops::focus_window_impl(win)
    }

    fn focus_app(&self, pid: i32) -> Result<(), AdapterError> {
        crate::system::app_ops::ensure_app_focused(pid)
    }

    fn launch_app(&self, id: &str, timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
        crate::system::launch::launch_app_impl(id, timeout_ms)
    }

    fn launch_app_with_options(
        &self,
        id: &str,
        options: &agent_desktop_core::launch_options::LaunchOptions,
        timeout_ms: u64,
    ) -> Result<WindowInfo, AdapterError> {
        crate::system::launch::launch_app_with_options_impl(id, options, timeout_ms)
    }

    fn process_state(
        &self,
        pid: i32,
    ) -> Result<agent_desktop_core::process_state::ProcessState, AdapterError> {
        crate::system::process_state::process_state_impl(pid)
    }

    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        crate::system::signals::supported_surfaces_impl()
    }

    fn capture_signal_baseline(
        &self,
        filter: &agent_desktop_core::signals::SignalFilter,
    ) -> Result<agent_desktop_core::signals::SignalBaseline, AdapterError> {
        crate::system::signals::capture_signal_baseline_impl(filter)
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

    fn list_displays(&self) -> Result<Vec<agent_desktop_core::DisplayInfo>, AdapterError> {
        crate::system::display::list_displays_impl()
    }

    fn screenshot(&self, target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> {
        match target {
            ScreenshotTarget::Window(pid) => crate::system::screenshot::capture_app(pid),
            ScreenshotTarget::Screen(idx) => crate::system::screenshot::capture_screen(idx),
            ScreenshotTarget::FullScreen => crate::system::screenshot::capture_screen(0),
        }
    }

    fn focused_window(&self) -> Result<Option<WindowInfo>, AdapterError> {
        let filter = WindowFilter {
            focused_only: true,
            app: None,
        };
        let windows = self.list_windows(&filter)?;
        Ok(windows.into_iter().next())
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

    fn resolve_window_strict(&self, win: &WindowInfo) -> Result<WindowInfo, AdapterError> {
        crate::system::window_resolve::resolve_window_strict(win)
    }

    fn window_op(&self, win: &WindowInfo, op: WindowOp) -> Result<(), AdapterError> {
        crate::system::window_ops::execute(win, op)
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
}
