use crate::{
    AdapterError, AdapterSession, AppInfo, CursorOverlayInstruction, Deadline,
    DismissAllNotificationsRequest, DismissNotificationRequest, ImageBuffer, InteractionLease,
    InteractionPolicy, KeyCombo, NotificationActionRequest, NotificationFilter, NotificationInfo,
    PermissionReport, PermissionState, ProcessIdentity, SessionAffinity, SignalBaseline,
    SignalFilter, WindowInfo, WindowOp, action_result::ActionResult, display_info::DisplayInfo,
    screenshot_target::ScreenshotTarget,
};

pub trait SystemOps: Send + Sync {
    fn present_cursor_overlay(
        &self,
        _instruction: &CursorOverlayInstruction,
    ) -> Result<(), AdapterError> {
        Ok(())
    }

    fn run_cursor_overlay_child(&self) -> Option<Result<(), AdapterError>> {
        None
    }

    fn acquire_interaction_lease(
        &self,
        _deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        Err(AdapterError::not_supported("acquire_interaction_lease"))
    }

    /// An adapter that never overrides this has not probed permissions at all,
    /// which is not the same fact as a user denying one. `Unknown` routes through
    /// `unknown_accessibility_means_unsupported` to `PLATFORM_NOT_SUPPORTED`
    /// rather than the misleading `PERM_DENIED`.
    fn permission_report(&self, _deadline: Deadline) -> Result<PermissionReport, AdapterError> {
        Ok(PermissionReport {
            accessibility: PermissionState::Unknown,
            screen_recording: PermissionState::Unknown,
            automation: PermissionState::NotRequired,
        })
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        true
    }

    /// Performs one renderer-accessibility activation mutation and returns.
    /// Readiness polling belongs to core after the interaction lease is dropped.
    fn activate_renderer_accessibility(
        &self,
        _process: ProcessIdentity,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported(
            "activate_renderer_accessibility",
        ))
    }

    fn request_permissions(
        &self,
        lease: &InteractionLease,
    ) -> Result<PermissionReport, AdapterError> {
        self.permission_report(lease.deadline())
    }

    /// Performs the platform-native focus/raise operation for the exact window.
    /// Core decides when focus is required; implementations must return only
    /// after that window is confirmed focused, or return an error.
    fn focus_window(
        &self,
        _win: &WindowInfo,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("focus_window"))
    }

    fn launch_app(
        &self,
        _id: &str,
        _options: &crate::launch_options::LaunchOptions,
        _lease: &InteractionLease,
    ) -> Result<crate::launch_result::LaunchResult, AdapterError> {
        Err(AdapterError::not_supported("launch_app"))
    }

    fn process_state(
        &self,
        _process: ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<crate::process_state::ProcessState, AdapterError> {
        Err(AdapterError::not_supported("process_state"))
    }

    fn supported_surfaces(&self) -> Vec<crate::adapter::SnapshotSurface> {
        Vec::new()
    }

    /// Opens adapter-native connection affinity for a persistent host.
    ///
    /// The returned session may retain platform connection state, but never a
    /// resolved element handle. Stateless command callers do not need to open a
    /// session. Adapters without persistent native state return
    /// `PLATFORM_NOT_SUPPORTED`.
    fn open_session(
        &self,
        _affinity: &SessionAffinity,
        _deadline: Deadline,
    ) -> Result<Box<dyn AdapterSession>, AdapterError> {
        Err(AdapterError::not_supported("open_session"))
    }

    /// Captures a point-in-time [`SignalBaseline`] snapshot,
    /// narrowed by `filter` when the caller already knows which app it cares
    /// about. `deadline` is one absolute budget shared by every native read in
    /// the capture; an adapter must return `TIMEOUT` rather than publish an
    /// observation completed at or after it. `wait --event` calls this once at
    /// wait-start and again on every poll, then diffs the two snapshots with
    /// `crate::diff_signals` — the adapter never decides what changed,
    /// only what the desktop looks like right now.
    fn capture_signal_baseline(
        &self,
        _filter: &SignalFilter,
        _deadline: Deadline,
    ) -> Result<SignalBaseline, AdapterError> {
        Err(AdapterError::not_supported("capture_signal_baseline"))
    }

    fn close_app(
        &self,
        _app: &AppInfo,
        _force: bool,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
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

    fn screenshot(
        &self,
        _target: ScreenshotTarget,
        _deadline: Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("screenshot"))
    }

    fn list_displays(&self, _deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_displays"))
    }

    fn focused_window(&self, _deadline: Deadline) -> Result<Option<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("focused_window"))
    }

    fn press_key_for_app(
        &self,
        _process: ProcessIdentity,
        _combo: &crate::KeyCombo,
        _policy: crate::InteractionPolicy,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("press_key_for_app"))
    }

    fn wait_for_menu(
        &self,
        _process: ProcessIdentity,
        _open: bool,
        _deadline: Deadline,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("wait_for_menu"))
    }

    fn window_op(
        &self,
        _win: &WindowInfo,
        _op: WindowOp,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("window_op"))
    }

    /// Resolves a live window by `WindowInfo.id`, corroborating the match against
    /// `pid` and, when present, `title`. Opaque ids must not be parsed as numeric
    /// outside the adapter; macOS uses `w-<kCGWindowNumber>`.
    fn resolve_window_strict(
        &self,
        _win: &WindowInfo,
        _deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Err(AdapterError::not_supported("resolve_window_strict"))
    }

    fn list_notifications(
        &self,
        _filter: &NotificationFilter,
        _policy: InteractionPolicy,
        _deadline: Deadline,
        _lease: Option<&InteractionLease>,
    ) -> Result<Vec<NotificationInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_notifications"))
    }

    fn dismiss_notification(
        &self,
        _request: DismissNotificationRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<NotificationInfo, AdapterError> {
        Err(AdapterError::not_supported("dismiss_notification"))
    }

    fn dismiss_all_notifications(
        &self,
        _request: DismissAllNotificationsRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<(Vec<NotificationInfo>, Vec<String>), AdapterError> {
        Err(AdapterError::not_supported("dismiss_all_notifications"))
    }

    fn notification_action(
        &self,
        _request: NotificationActionRequest<'_>,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("notification_action"))
    }
}

#[cfg(test)]
#[path = "system_tests.rs"]
mod tests;
