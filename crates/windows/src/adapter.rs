use agent_desktop_core::{
    AccessibilityNode, ActionOps, AdapterError, AppInfo, Deadline, ElementState, InputOps,
    InteractionLease, LiveElement, MouseEvent, NativeHandle, ObservationOps, ObservationRequest,
    ObservationRoot, ProcessIdentity, Rect, RefEntry, TreeOptions, WindowFilter, WindowInfo,
};
use std::collections::HashSet;
use std::sync::{Mutex, MutexGuard, PoisonError};

/// The Windows adapter.
///
/// Carries the renderer-activation state the observation loop needs: which
/// processes have **already had** the Chromium settle run for them. Core's
/// loop calls `activate_renderer_accessibility` (the settle) then retries
/// `observe_tree`; the adapter must distinguish a process's pre-settle shell
/// (which re-arms the loop) from its post-settle still-thin tree (which
/// eventually returns the guidance error instead of looping forever). The
/// state is keyed per process, not a single flag: an adapter instance is
/// reused across a batch or an FFI session, so one process settling must
/// never suppress another process's activation.
pub struct WindowsAdapter {
    renderer_activation_attempted: Mutex<HashSet<ProcessIdentity>>,
}

impl WindowsAdapter {
    pub fn new() -> Self {
        Self {
            renderer_activation_attempted: Mutex::new(HashSet::new()),
        }
    }

    /// Records that the Chromium settle has run for `process`, scoped to its
    /// generation (the process-instance token) so a recycled pid never
    /// inherits another process's settle state.
    pub(crate) fn note_renderer_activation_attempted(&self, process: ProcessIdentity) {
        self.activation_set().insert(process);
    }

    /// Whether the Chromium settle has already run for `process`.
    pub(crate) fn renderer_activation_attempted(&self, process: &ProcessIdentity) -> bool {
        self.activation_set().contains(process)
    }

    fn activation_set(&self) -> MutexGuard<'_, HashSet<ProcessIdentity>> {
        self.renderer_activation_attempted
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
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
    /// path the binary's `snapshot` uses (mirroring
    /// `crates/macos/src/tree/adapter.rs`). `get_subtree` has no live caller
    /// on any platform and stays unimplemented.
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

    /// Re-resolves a stored ref to a live element, fail-closed - the
    /// drill-down root `snapshot --root @ref` needs.
    fn resolve_element_strict(
        &self,
        entry: &RefEntry,
        deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        crate::tree::resolve::resolve_element_strict(entry, deadline)
    }

    fn resolve_locator_anchor(
        &self,
        entry: &RefEntry,
        deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        crate::tree::resolve_anchor::resolve_locator_anchor(entry, deadline)
    }

    /// The five live readers are projections over one shared single-element
    /// read (`crate::tree::live_read`), which corroborates the verified
    /// process token before answering and fails retryable when an essential
    /// slot could not be read, and a dead provider token never satisfies
    /// completeness (A14-9).
    fn get_live_value(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<String>, AdapterError> {
        let read = crate::tree::live_read::read_live_element(handle, deadline)?;
        Ok(crate::tree::live_read::live_value(&read))
    }

    fn get_live_state(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        let read = crate::tree::live_read::read_live_element(handle, deadline)?;
        Ok(Some(crate::tree::live_read::live_state(&read)?))
    }

    fn get_live_actions(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        let read = crate::tree::live_read::read_live_element(handle, deadline)?;
        Ok(Some(crate::tree::live_read::live_actions(&read)?))
    }

    fn get_live_element(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        let read = crate::tree::live_read::read_live_element(handle, deadline)?;
        crate::tree::live_read::live_element(&read)
    }

    fn get_element_bounds(
        &self,
        handle: &NativeHandle,
        deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        let read = crate::tree::live_read::read_live_element(handle, deadline)?;
        Ok(crate::tree::live_read::live_bounds(&read))
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        point: agent_desktop_core::Point,
        deadline: Deadline,
    ) -> Result<agent_desktop_core::hit_test::HitTestResult, AdapterError> {
        crate::tree::hit_test::hit_test_impl(handle, point, deadline)
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
impl ActionOps for WindowsAdapter {
    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: agent_desktop_core::action_request::ActionRequest,
        lease: &InteractionLease,
    ) -> Result<agent_desktop_core::ActionResult, AdapterError> {
        crate::actions::dispatch::execute_action_impl(handle, request, lease)
    }

    fn scroll_into_view(
        &self,
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        crate::actions::scroll_into_view::scroll_into_view_impl(handle, lease)
    }
}
impl InputOps for WindowsAdapter {
    fn mouse_event(&self, event: MouseEvent, lease: &InteractionLease) -> Result<(), AdapterError> {
        crate::input::mouse::synthesize_mouse(event, lease.deadline())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{SnapshotSurface, SystemOps};

    /// The surfaces gate: the adapter advertises exactly the surfaces it can
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
        let process = ProcessIdentity::new(agent_desktop_core::ProcessId::new(1), "gen-1");

        assert!(!adapter.renderer_activation_attempted(&process));

        adapter.note_renderer_activation_attempted(process.clone());
        assert!(adapter.renderer_activation_attempted(&process));
    }

    /// Finding 2: a single global flag would let one process's settle
    /// suppress another process's activation forever. The state must be
    /// scoped so a second, unrelated process is unaffected by the first.
    #[test]
    fn renderer_activation_state_is_scoped_per_process_not_global() {
        let adapter = WindowsAdapter::new();
        let first = ProcessIdentity::new(agent_desktop_core::ProcessId::new(1), "gen-1");
        let second = ProcessIdentity::new(agent_desktop_core::ProcessId::new(2), "gen-1");

        adapter.note_renderer_activation_attempted(first.clone());

        assert!(adapter.renderer_activation_attempted(&first));
        assert!(!adapter.renderer_activation_attempted(&second));
    }
}
