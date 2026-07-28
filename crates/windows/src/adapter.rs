use agent_desktop_core::{ActionOps, InputOps, ObservationOps};

pub struct WindowsAdapter;

impl WindowsAdapter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ObservationOps for WindowsAdapter {}
impl ActionOps for WindowsAdapter {}
impl InputOps for WindowsAdapter {}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{AppError, CommandContext, ErrorCode, SnapshotSurface, SystemOps};

    #[test]
    fn snapshot_surfaces_fail_closed_until_windows_implements_them() {
        let adapter = WindowsAdapter::new();
        assert!(adapter.supported_surfaces().is_empty());

        let error = agent_desktop_core::commands::snapshot::execute(
            agent_desktop_core::commands::snapshot::SnapshotArgs {
                app: None,
                window_id: None,
                max_depth: 1,
                include_bounds: false,
                interactive_only: false,
                compact: true,
                surface: SnapshotSurface::Window,
                skeleton: false,
                root_ref: None,
                snapshot_id: None,
            },
            &adapter,
            &CommandContext::default(),
        )
        .expect_err("an unimplemented surface must fail at validation");

        let AppError::Adapter(error) = error else {
            panic!("surface validation must return an adapter error")
        };
        assert_eq!(error.code, ErrorCode::PlatformNotSupported);
        assert!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("supported_surfaces"))
                .and_then(|surfaces| surfaces.as_array())
                .is_some_and(Vec::is_empty)
        );
    }
}
