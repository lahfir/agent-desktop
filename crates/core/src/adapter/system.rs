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

    fn launch_app_with_options(
        &self,
        id: &str,
        options: &crate::launch_options::LaunchOptions,
        timeout_ms: u64,
    ) -> Result<WindowInfo, AdapterError> {
        if !options.args.is_empty()
            || options.cwd.is_some()
            || !options.env.is_empty()
            || !options.attach
        {
            return Err(AdapterError::not_supported("launch_app_with_options"));
        }
        self.launch_app(id, timeout_ms)
    }

    fn process_state(&self, _pid: i32) -> Result<crate::process_state::ProcessState, AdapterError> {
        Err(AdapterError::not_supported("process_state"))
    }

    fn supported_surfaces(&self) -> Vec<crate::adapter::SnapshotSurface> {
        vec![crate::adapter::SnapshotSurface::Window]
    }

    /// Captures a point-in-time [`crate::signals::SignalBaseline`] snapshot,
    /// narrowed by `filter` when the caller already knows which app it cares
    /// about. `wait --event` calls this once at wait-start and again on every
    /// poll, then diffs the two snapshots with `crate::signals::diff_signals`
    /// — the adapter never decides what changed, only what the desktop looks
    /// like right now.
    fn capture_signal_baseline(
        &self,
        _filter: &crate::signals::SignalFilter,
    ) -> Result<crate::signals::SignalBaseline, AdapterError> {
        Err(AdapterError::not_supported("capture_signal_baseline"))
    }

    /// Opens adapter-native session affinity for a host that outlives a
    /// single command (an FFI embedder, a future daemon) — the landing zone
    /// for a Windows COM-MTA apartment thread or a Linux D-Bus connection
    /// before those adapters exist. `affinity.session_id` lets the caller
    /// tie the native connection's lifetime to a CLI-level session (see
    /// [`crate::session::SessionManifest`]). The returned session may hold
    /// native connection state but must never hold a resolved element
    /// handle — commands keep resolving elements per call from a
    /// `RefEntry`, exactly as they do today. Nothing in the CLI/dispatch
    /// path calls this yet; the stateless request-per-command flow is
    /// unaffected until a persistent host opts in. Adapters with no native
    /// connection state to manage return `not_supported`.
    fn open_session(
        &self,
        _affinity: &crate::session_affinity::SessionAffinity,
    ) -> Result<Box<dyn crate::adapter_session::AdapterSession>, AdapterError> {
        Err(AdapterError::not_supported("open_session"))
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

    /// Resolves a live window by `WindowInfo.id`, corroborating the match against
    /// `pid` and, when present, `title`. Opaque ids must not be parsed as numeric
    /// outside the adapter; macOS uses `w-<kCGWindowNumber>`.
    fn resolve_window_strict(&self, _win: &WindowInfo) -> Result<WindowInfo, AdapterError> {
        Err(AdapterError::not_supported("resolve_window_strict"))
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

#[cfg(test)]
#[path = "system_tests.rs"]
mod tests;
