use agent_desktop_core::{
    AdapterError, AdapterSession, Deadline, InteractionLease, PermissionReport, SessionAffinity,
    SystemOps,
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
    use agent_desktop_core::ErrorCode;

    #[test]
    fn unknown_accessibility_matches_the_cli_outcome_of_platform_not_supported() {
        let adapter = WindowsAdapter::new();

        assert!(adapter.unknown_accessibility_means_unsupported());

        let error = adapter
            .list_displays(Deadline::after(1_000).unwrap())
            .unwrap_err();
        assert_eq!(error.code, ErrorCode::PlatformNotSupported);
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
