use agent_desktop_core::{
    AccessibilityNode, ActionOps, AdapterError, AppInfo, ClipboardContent, ClipboardFormat,
    Deadline, DragParams, ElementState, InputOps, InteractionLease, KeyCombo, LiveElement,
    MouseEvent, NativeHandle, ObservationOps, ObservationRequest, ObservationRoot, ProcessIdentity,
    Rect, RefEntry, SnapshotSurface, TreeOptions, WindowFilter, WindowInfo,
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
        deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        crate::system::window_ops::list_windows_live(filter, deadline)
    }

    fn list_apps(&self, deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        crate::system::app_ops::list_apps_live(deadline)
    }

    /// App-less shell-surface resolution: the read-only half of the shell
    /// surface seam. The shell-surface resolver in `system::shell_surface`
    /// owns the reach mechanics and answers `None` for a surface that is not
    /// presented; this is the translation that turns that absence into the
    /// answer a `snapshot --surface` caller can act on, while a build refusal
    /// passes through unchanged.
    fn resolve_shell_surface(
        &self,
        surface: SnapshotSurface,
        deadline: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        crate::system::shell_surface::resolve_surface(surface, deadline)?
            .ok_or_else(|| crate::system::shell_surface::not_open_error(surface))
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

    /// A standalone key edge has no daemon to own the hold across a request
    /// boundary, so it rejects the same way macOS's `key_event` does -
    /// the atomic `press`/`type` composers live in the
    /// `execute_action` physical legs, not here.
    fn key_event(
        &self,
        combo: &KeyCombo,
        down: bool,
        _lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        crate::input::keyboard::reject_standalone_key_state(combo, down)
    }

    fn drag(&self, params: DragParams, lease: &InteractionLease) -> Result<(), AdapterError> {
        crate::input::drag::synthesize_drag(params, lease.deadline())
    }

    fn clear_clipboard(&self, lease: &InteractionLease) -> Result<(), AdapterError> {
        crate::input::clipboard::clear(lease.deadline())
    }

    fn get_clipboard_content(
        &self,
        format: ClipboardFormat,
        deadline: Deadline,
    ) -> Result<Option<ClipboardContent>, AdapterError> {
        crate::input::clipboard::get_clipboard_content(format, deadline)
    }

    fn set_clipboard_content(
        &self,
        content: &ClipboardContent,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        crate::input::clipboard::set_content(content, lease.deadline())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::{InteractionLease, SnapshotSurface, SystemOps};

    /// `key_event` through the trait, exactly as FFI reaches it: a standalone
    /// edge has no daemon to own the hold, so it must reject with zero
    /// synthesis regardless of `down`'s value.
    #[test]
    fn key_event_rejects_a_standalone_edge_through_the_trait() {
        let adapter = WindowsAdapter::new();
        let deadline = Deadline::after(5_000).expect("bounded deadline");
        let lease = InteractionLease::guarded(deadline, ()).expect("lease");
        let combo = KeyCombo {
            key: "a".into(),
            modifiers: Vec::new(),
        };

        let error = InputOps::key_event(&adapter, &combo, true, &lease)
            .expect_err("a standalone key edge must reject");

        assert_eq!(
            error.code,
            agent_desktop_core::ErrorCode::ActionNotSupported
        );
        let details = error.details.expect("standalone error carries details");
        assert_eq!(details["raw_input_emitted"], false);
        assert_eq!(details["requires_daemon_owned_transaction"], true);
    }

    /// The surfaces gate: the adapter advertises exactly the surfaces it can
    /// observe - a named window, the focused window, a Chromium modal
    /// classified as a sheet, an open application menu, and the shell kinds
    /// the kind table resolves on this build. Core validates the requested
    /// surface against this list before the adapter is ever called, so this
    /// advertisement is what makes `snapshot` end to end possible, and it is
    /// the "advertised" side of the advertise/resolve/emit equality the live
    /// tests pin.
    #[test]
    fn supported_surfaces_advertises_window_focused_sheet_menu_and_shell_kinds() {
        let adapter = WindowsAdapter::new();
        assert_eq!(
            adapter.supported_surfaces(),
            vec![
                SnapshotSurface::Window,
                SnapshotSurface::Focused,
                SnapshotSurface::Sheet,
                SnapshotSurface::Menu,
                SnapshotSurface::StartMenu,
                SnapshotSurface::Taskbar,
                SnapshotSurface::SystemTray,
                SnapshotSurface::SystemTrayOverflow,
                SnapshotSurface::ActionCenter,
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
