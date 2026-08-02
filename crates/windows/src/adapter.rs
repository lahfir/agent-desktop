use agent_desktop_core::{
    AccessibilityNode, ActionOps, AdapterError, AppInfo, Deadline, InputOps, NativeHandle,
    ObservationOps, ObservationRequest, ObservationRoot, RefEntry, TreeOptions, WindowFilter,
    WindowInfo,
};
use std::sync::atomic::{AtomicBool, Ordering};

/// The Windows adapter.
///
/// Carries the renderer-activation state KTD7 needs: whether the Chromium
/// settle has **already run** for this process. Core's loop calls
/// `activate_renderer_accessibility` (the settle) then retries
/// `observe_tree`; the adapter must distinguish the pre-settle shell (which
/// re-arms the loop) from the post-settle still-thin tree (which returns the
/// guidance error instead of looping forever). A fresh adapter per CLI
/// invocation makes an instance flag the honest carrier.
pub struct WindowsAdapter {
    renderer_activation_attempted: AtomicBool,
}

impl WindowsAdapter {
    pub fn new() -> Self {
        Self {
            renderer_activation_attempted: AtomicBool::new(false),
        }
    }

    pub(crate) fn note_renderer_activation_attempted(&self) {
        self.renderer_activation_attempted
            .store(true, Ordering::Relaxed);
    }

    pub(crate) fn renderer_activation_attempted(&self) -> bool {
        self.renderer_activation_attempted.load(Ordering::Relaxed)
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
        crate::tree::observe::observe_tree(root, request, self)
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

    /// Re-resolves a stored ref to a live element, fail-closed (KTD9) - the
    /// drill-down root `snapshot --root @ref` needs.
    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        crate::tree::resolve::resolve_element_strict(entry, deadline)
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

    /// U6/U7's surfaces gate: the adapter advertises exactly the surfaces it can
    /// observe - a named window, the focused window, and a Chromium modal
    /// classified as a sheet. Core validates the requested surface against this
    /// list before the adapter is ever called, so this advertisement is what
    /// makes `snapshot` end to end possible.
    #[test]
    fn supported_surfaces_advertises_window_focused_and_sheet() {
        let adapter = WindowsAdapter::new();
        assert_eq!(
            adapter.supported_surfaces(),
            vec![
                SnapshotSurface::Window,
                SnapshotSurface::Focused,
                SnapshotSurface::Sheet,
            ]
        );
    }

    #[test]
    fn renderer_activation_state_starts_unattempted_and_notes_once() {
        let adapter = WindowsAdapter::new();
        assert!(!adapter.renderer_activation_attempted());

        adapter.note_renderer_activation_attempted();
        assert!(adapter.renderer_activation_attempted());
    }
}
