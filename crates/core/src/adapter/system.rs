use crate::{
    PermissionReport, PermissionState,
    action::{KeyCombo, WindowOp},
    action_result::ActionResult,
    display_info::DisplayInfo,
    error::AdapterError,
    image_buffer::ImageBuffer,
    node::WindowInfo,
    notification::{NotificationFilter, NotificationIdentity, NotificationInfo},
    screenshot_target::ScreenshotTarget,
};

pub trait SystemOps: Send + Sync {
    fn permission_report(&self) -> PermissionReport {
        PermissionReport {
            accessibility: PermissionState::Denied {
                suggestion: "Platform adapter not available".into(),
            },
            screen_recording: PermissionState::Unknown,
            automation: PermissionState::NotRequired,
        }
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        true
    }

    fn request_permissions(&self) -> PermissionReport {
        self.permission_report()
    }

    fn focus_window(&self, _win: &WindowInfo) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_window"))
    }

    /// Brings the application owning `pid` to the foreground. Best-effort guard
    /// invoked before physical (cursor/keyboard) input that targets a known
    /// element, so synthetic events land on the intended window rather than
    /// whatever happens to be frontmost.
    fn focus_app(&self, _pid: i32) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_app"))
    }

    fn launch_app(&self, _id: &str, _timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
        Err(AdapterError::not_supported("launch_app"))
    }

    fn close_app(&self, _id: &str, _force: bool) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("close_app"))
    }

    /// Reports whether closing `identifier` would terminate a process the OS
    /// depends on (window server, login session, shell). The set is
    /// inherently platform-specific, so each adapter owns its own list;
    /// core only asks. The default denies nothing.
    fn is_protected_process(&self, _identifier: &str) -> bool {
        false
    }

    /// Reports whether `combo` is a platform-dangerous keyboard shortcut that
    /// should be refused unless the caller explicitly forces it (for example
    /// macOS Cmd+Q quit, Ctrl+Cmd+Q lock, Cmd+Alt+Esc force-quit). Which
    /// combos are dangerous — and how key names alias to physical keys — is
    /// platform-specific, so each adapter owns its own list; core only asks
    /// and lets the caller override via `--force`. The default blocks nothing,
    /// leaving the decision entirely to the calling agent.
    fn is_blocked_combo(&self, _combo: &KeyCombo) -> bool {
        false
    }

    fn screenshot(&self, _target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("screenshot"))
    }

    fn list_displays(&self) -> Result<Vec<DisplayInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_displays"))
    }

    fn focused_window(&self) -> Result<Option<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("focused_window"))
    }

    fn press_key_for_app(
        &self,
        _app_name: &str,
        _combo: &crate::action::KeyCombo,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("press_key_for_app"))
    }

    fn wait_for_menu(&self, _pid: i32, _open: bool, _timeout_ms: u64) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("wait_for_menu"))
    }

    fn window_op(&self, _win: &WindowInfo, _op: WindowOp) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("window_op"))
    }

    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_notifications"))
    }

    fn dismiss_notification(
        &self,
        _index: usize,
        _app_filter: Option<&str>,
    ) -> Result<NotificationInfo, AdapterError> {
        Err(AdapterError::not_supported("dismiss_notification"))
    }

    fn dismiss_all_notifications(
        &self,
        _app_filter: Option<&str>,
    ) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
        Err(AdapterError::not_supported("dismiss_all_notifications"))
    }

    /// Press a named action button on the notification at `index`.
    ///
    /// `identity` lets the caller pin the targeted notification to an
    /// expected app / title fingerprint. Notification Center reindexes
    /// entries between listings, so index-only targeting can press the
    /// wrong button if a notification arrives or is dismissed between
    /// `list_notifications` and this call. When any identity field is
    /// `Some`, implementations must return
    /// `ErrorCode::NotificationNotFound` if the row at `index` does not
    /// match. Passing an empty identity (or `None`) preserves legacy
    /// index-only behavior for callers that reconcile themselves.
    fn notification_action(
        &self,
        _index: usize,
        _identity: Option<&NotificationIdentity>,
        _action_name: &str,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("notification_action"))
    }
}
