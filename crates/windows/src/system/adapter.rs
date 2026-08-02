use agent_desktop_core::{
    AdapterError, AdapterSession, Deadline, InteractionLease, ObservationOps, PermissionReport,
    SessionAffinity, SystemOps, WindowFilter, WindowInfo,
};

use crate::adapter::WindowsAdapter;

impl SystemOps for WindowsAdapter {
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

    /// The focused window is the focused-only filter's first result, composed
    /// from `list_windows` rather than a second native path (KTD10, mirroring
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
}
