use agent_desktop_core::{
    AccessibilityNode, ActionOps, AdapterError, AppInfo, Deadline, InputOps, ObservationOps,
    ObservationRequest, ObservationRoot, TreeOptions, WindowFilter, WindowInfo,
};

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

impl ObservationOps for WindowsAdapter {
    fn observe_tree(
        &self,
        root: ObservationRoot<'_>,
        request: &ObservationRequest,
    ) -> Result<agent_desktop_core::ObservedTree, AdapterError> {
        crate::tree::observe::observe_tree(root, request)
    }

    /// The FFI legacy entrypoint: a thin wrapper over the same `observe_tree`
    /// path (KTD1, mirroring `crates/macos/src/tree/adapter.rs`). `get_subtree`
    /// has no live caller on any platform and stays unimplemented.
    fn get_tree(
        &self,
        window: &WindowInfo,
        options: &TreeOptions,
        deadline: Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        self.observe_tree(
            ObservationRoot::Window(window),
            &ObservationRequest::snapshot(options, deadline),
        )?
        .into_accessibility_tree()
    }

    fn list_windows(
        &self,
        filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        crate::system::window_ops::list_windows_live(filter)
    }

    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        crate::system::app_ops::list_apps_live()
    }
}
impl ActionOps for WindowsAdapter {}
impl InputOps for WindowsAdapter {}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{SnapshotSurface, SystemOps};

    /// U6's surfaces gate: the adapter advertises exactly the surfaces it can
    /// observe - a named window and the focused window. Core validates the
    /// requested surface against this list before the adapter is ever called,
    /// so this advertisement is what makes `snapshot` end to end possible; the
    /// old empty-set fail-closed pin is retired because the adapter now
    /// implements the observation path.
    #[test]
    fn supported_surfaces_advertises_window_and_focused() {
        let adapter = WindowsAdapter::new();
        assert_eq!(
            adapter.supported_surfaces(),
            vec![SnapshotSurface::Window, SnapshotSurface::Focused]
        );
    }
}
