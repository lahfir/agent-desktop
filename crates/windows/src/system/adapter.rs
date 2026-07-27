use agent_desktop_core::{AdapterError, Deadline, InteractionLease, PermissionReport, SystemOps};

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
