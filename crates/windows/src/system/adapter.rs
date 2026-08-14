use agent_desktop_core::{
    ActionResult, AdapterError, AdapterSession, AppInfo, Deadline, DisplayInfo, ImageBuffer,
    InteractionLease, InteractionPolicy, KeyCombo, ObservationOps, PermissionReport,
    ProcessIdentity, ScreenshotTarget, SessionAffinity, SnapshotSurface, SystemOps, WindowFilter,
    WindowInfo, WindowOp, launch_options::LaunchOptions, process_state::ProcessState,
};

use crate::adapter::WindowsAdapter;

impl SystemOps for WindowsAdapter {
    /// The interaction lease: enough to unblock renderer activation.
    ///
    /// Core's activation loop acquires a lease before activating, and the
    /// trait default fails closed - so without this override the first settle
    /// would die `PLATFORM_NOT_SUPPORTED`. Full cross-process lease semantics
    /// are owned by the later input work on this crate.
    fn acquire_interaction_lease(
        &self,
        deadline: Deadline,
    ) -> Result<InteractionLease, AdapterError> {
        InteractionLease::guarded(deadline, ())
    }

    /// The Chromium settle: connecting to UI Automation already triggered the
    /// renderer's asynchronous accessibility build (A1-4/A1-5), so activation
    /// is the connection-plus-settle itself. A short backoff lets the build
    /// start landing before core's loop re-walks; core's own retry backoff is
    /// what turns this into a real wait, and the adapter notes that the
    /// settle ran **for this process** so a same-process retry keeps only the
    /// walk in the loop, never a second activation, while an unrelated
    /// process observed through the same adapter still gets its own pass.
    fn activate_renderer_accessibility(
        &self,
        process: ProcessIdentity,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        self.note_renderer_activation_attempted(process);
        let settled = lease.deadline().remaining();
        if settled.is_zero() {
            return Ok(());
        }
        std::thread::sleep(settled.min(std::time::Duration::from_millis(50)));
        Ok(())
    }

    fn permission_report(&self, deadline: Deadline) -> Result<PermissionReport, AdapterError> {
        crate::system::permissions::report(deadline)
    }

    fn request_permissions(
        &self,
        lease: &InteractionLease,
    ) -> Result<PermissionReport, AdapterError> {
        crate::system::permissions::request_report(lease.deadline())
    }

    fn unknown_accessibility_means_unsupported(&self) -> bool {
        true
    }

    /// The surfaces this adapter can observe: a named window, the focused
    /// window, and a Chromium modal-as-sheet. Core validates the requested
    /// surface against this list before the adapter is ever called (the mirror
    /// of macOS `supported_surfaces`). The sheet surface resolves the focused
    /// window when it is itself modal (`WindowIsModal`), reaching a Chromium
    /// modal through the sheet surface.
    fn supported_surfaces(&self) -> Vec<SnapshotSurface> {
        vec![
            SnapshotSurface::Window,
            SnapshotSurface::Focused,
            SnapshotSurface::Sheet,
        ]
    }

    /// The focused window is the focused-only filter's first result, composed
    /// from `list_windows` rather than a second native path (mirroring
    /// `crates/macos/src/system/adapter.rs:142-149`). Whatever HWND-shape a
    /// host presents, it maps to the same identity `list_windows` reports.
    fn focused_window(&self, deadline: Deadline) -> Result<Option<WindowInfo>, AdapterError> {
        let filter = WindowFilter {
            focused_only: true,
            app: None,
        };
        let windows = self.list_windows(&filter, deadline)?;
        Ok(windows.into_iter().next())
    }

    /// Process liveness for the shared `ProcessState` contract. Returns the
    /// raw classification only — core's two-signal gate owns any upgrade to
    /// `APP_UNRESPONSIVE` (A21-3, A21-4).
    fn process_state(
        &self,
        process: ProcessIdentity,
        deadline: Deadline,
    ) -> Result<ProcessState, AdapterError> {
        crate::system::process_state::process_state_impl(process, deadline)
    }

    /// Session- and shell-critical image names; exact match, case-insensitive.
    fn is_protected_process(&self, identifier: &str) -> bool {
        crate::system::app_ops::is_protected_process(identifier)
    }

    /// `CreateProcessW` launch with system-directory-only bare-name resolution
    /// and ToolHelp attach detection (A21-1, A21-8).
    fn launch_app(
        &self,
        id: &str,
        options: &LaunchOptions,
        lease: &InteractionLease,
    ) -> Result<agent_desktop_core::launch_result::LaunchResult, AdapterError> {
        crate::system::launch::launch_app_impl(id, options, lease.deadline())
    }

    /// Verified termination: `Ok(())` only after the process is observed gone
    /// (A21-3). Graceful posts `WM_CLOSE`; force calls `TerminateProcess`.
    fn close_app(
        &self,
        app: &AppInfo,
        force: bool,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        crate::system::close::close_app_impl(app, force, lease.deadline())
    }

    /// `SetWindowPos`/`ShowWindow` with Win32 placement re-read verification
    /// (A21-5); never UIA `IsOffscreen`/`-32000`.
    fn window_op(
        &self,
        win: &WindowInfo,
        op: WindowOp,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        crate::system::window_op::window_op_impl(win, op, lease.deadline())
    }

    fn resolve_window_strict(
        &self,
        win: &WindowInfo,
        deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        crate::system::window_resolve::resolve_window_strict(win, deadline)
    }

    fn focus_window(&self, win: &WindowInfo, lease: &InteractionLease) -> Result<(), AdapterError> {
        crate::system::window_activate::focus_window(win, lease)
    }

    fn list_displays(&self, deadline: Deadline) -> Result<Vec<DisplayInfo>, AdapterError> {
        crate::system::display::list_displays_live(deadline)
    }

    #[cfg(target_os = "windows")]
    fn screenshot(
        &self,
        target: ScreenshotTarget,
        deadline: Deadline,
    ) -> Result<ImageBuffer, AdapterError> {
        crate::system::screenshot::screenshot(target, deadline)
    }

    /// Verify-only composition over the keyboard primitive. Core's headed
    /// path already activated; this never raises (A9-3: SendInput has no
    /// per-pid targeting).
    fn press_key_for_app(
        &self,
        process: ProcessIdentity,
        combo: &KeyCombo,
        policy: InteractionPolicy,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        crate::system::key_dispatch::press_for_app_impl(process, combo, policy, lease.deadline())
    }

    /// The core default blocks nothing; Windows ships its own dangerous-combo
    /// list so the skill-documented guard actually enforces on this
    /// platform. `--force` is honored entirely in core.
    fn is_blocked_combo(&self, combo: &KeyCombo) -> bool {
        crate::input::blocked_combo::is_blocked(combo)
    }

    fn open_session(
        &self,
        _affinity: &SessionAffinity,
        deadline: Deadline,
    ) -> Result<Box<dyn AdapterSession>, AdapterError> {
        Ok(Box::new(crate::system::session::open(deadline)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_accessibility_is_unsupported_so_cli_and_ffi_agree() {
        use agent_desktop_core::PermissionState;

        const UNRECOGNIZED_UIA_HRESULT: i32 = 0x8000_4005_u32 as i32;

        let adapter = WindowsAdapter::new();
        assert!(adapter.unknown_accessibility_means_unsupported());

        assert_eq!(
            crate::system::permissions::map_uia_access(UNRECOGNIZED_UIA_HRESULT),
            PermissionState::Unknown
        );
    }

    #[test]
    fn open_session_returns_a_live_session_instead_of_not_supported() {
        let affinity = SessionAffinity {
            session_id: Some("windows-com-session".into()),
        };

        let session = WindowsAdapter::new()
            .open_session(&affinity, Deadline::after(5_000).unwrap())
            .expect("windows must open an adapter session instead of failing closed");

        session.close().expect("a fresh session must close cleanly");
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn permission_report_through_the_trait_probes_instead_of_defaulting() {
        use agent_desktop_core::PermissionState;

        let report =
            SystemOps::permission_report(&WindowsAdapter::new(), Deadline::after(5_000).unwrap())
                .unwrap();

        assert_eq!(report.automation, PermissionState::NotRequired);
        assert!(matches!(
            report.accessibility,
            PermissionState::Granted | PermissionState::Denied { .. }
        ));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn the_activation_path_acquires_a_windows_lease_and_settles() {
        let adapter = WindowsAdapter::new();
        let lease = SystemOps::acquire_interaction_lease(&adapter, Deadline::after(5_000).unwrap())
            .expect("the Windows lease override must not show up as not_supported");

        SystemOps::activate_renderer_accessibility(
            &adapter,
            ProcessIdentity::new(std::process::id(), "test"),
            &lease,
        )
        .expect("activation is the settle, which succeeds");
    }

    /// The sheet surface specifically, reached through the `SystemOps` trait
    /// object rather than the inherent method. The full advertised vector is
    /// asserted by `supported_surfaces_advertises_window_focused_and_sheet`.
    #[test]
    fn sheet_is_among_the_surfaces_advertised_through_the_trait() {
        use agent_desktop_core::{SnapshotSurface, SystemOps as _};

        let surfaces = WindowsAdapter::new().supported_surfaces();
        assert!(surfaces.contains(&SnapshotSurface::Sheet));
    }

    /// Pins that `is_blocked_combo` is actually overridden here, reached
    /// through the trait object exactly as core calls it: the default blocks
    /// nothing, so an un-wired override would be indistinguishable from
    /// "nothing is dangerous".
    #[test]
    fn is_blocked_combo_is_wired_to_the_windows_dangerous_list_through_the_trait() {
        use agent_desktop_core::{KeyCombo, Modifier, SystemOps as _};

        let adapter = WindowsAdapter::new();
        let dangerous = KeyCombo {
            key: "f4".into(),
            modifiers: vec![Modifier::Alt],
        };
        let harmless = KeyCombo {
            key: "c".into(),
            modifiers: vec![Modifier::Ctrl],
        };

        assert!(adapter.is_blocked_combo(&dangerous));
        assert!(!adapter.is_blocked_combo(&harmless));
    }
}
