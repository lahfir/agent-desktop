use agent_desktop_core::{
    adapter::{
        LiveElement, NativeHandle, ObservationOps, SnapshotSurface, TreeOptions, WindowFilter,
    },
    element_state::ElementState,
    error::AdapterError,
    node::{AccessibilityNode, AppInfo, Rect, SurfaceInfo, WindowInfo},
    refs::RefEntry,
};
use rustc_hash::FxHashSet;

use crate::adapter::{MacOSAdapter, with_borrowed_ax_element};

impl ObservationOps for MacOSAdapter {
    fn get_tree(
        &self,
        win: &WindowInfo,
        opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        let el = match opts.surface {
            SnapshotSurface::Window => crate::system::window_resolve::window_element_for_info(win)?,
            SnapshotSurface::Focused => crate::tree::surfaces::focused_surface_for_pid(win.pid)
                .ok_or_else(|| AdapterError::internal("No focused surface found"))?,
            SnapshotSurface::Menu => crate::tree::surfaces::menu_element_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No open context menu"))?,
            SnapshotSurface::Menubar => crate::tree::surfaces::menubar_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No menu bar found"))?,
            SnapshotSurface::Sheet => crate::tree::surfaces::sheet_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No open sheet"))?,
            SnapshotSurface::Popover => crate::tree::surfaces::popover_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No visible popover"))?,
            SnapshotSurface::Alert => crate::tree::surfaces::alert_for_pid(win.pid)
                .ok_or_else(|| AdapterError::element_not_found("No open alert or dialog"))?,
            _ => return Err(AdapterError::not_supported("snapshot surface")),
        };
        let mut visited = FxHashSet::default();
        let context = crate::tree::TreeBuildContext::for_pid(win.pid, opts.include_bounds);
        crate::tree::build_subtree(
            &el,
            0,
            0,
            opts.max_depth,
            &mut visited,
            opts.skeleton,
            &context,
        )
        .ok_or_else(|| AdapterError::internal("Empty AX tree for surface"))
    }

    fn resolve_element_strict(&self, entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        crate::tree::resolve::resolve_element_impl(entry)
    }

    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        crate::tree::resolve::resolve_element_with_timeout(entry, timeout)
    }

    fn list_windows(&self, filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        crate::system::window_list::list_windows_impl(filter)
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        crate::system::app_list::list_apps_impl()
    }

    fn list_surfaces(&self, pid: i32) -> Result<Vec<SurfaceInfo>, AdapterError> {
        Ok(crate::tree::surfaces::list_surfaces_for_pid(pid))
    }

    fn resolve_query(
        &self,
        query: &agent_desktop_core::locator::LocatorQuery,
        scope: Option<&NativeHandle>,
        pid: i32,
    ) -> Result<Vec<NativeHandle>, AdapterError> {
        crate::tree::query::resolve_query_impl(query, pid, scope)
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        point: agent_desktop_core::action::Point,
    ) -> Result<agent_desktop_core::hit_test::HitTestResult, AdapterError> {
        crate::tree::hit_test::hit_test_impl(handle, point)
    }

    fn get_live_name_evidence(
        &self,
        handle: &NativeHandle,
    ) -> Result<agent_desktop_core::accname::NameEvidence, AdapterError> {
        Ok(with_borrowed_ax_element(
            handle,
            crate::tree::name_evidence::name_evidence_impl,
        ))
    }

    fn get_live_value(&self, handle: &NativeHandle) -> Result<Option<String>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(with_borrowed_ax_element(
                handle,
                crate::tree::copy_value_typed,
            ))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_value"))
    }

    fn get_live_state(&self, handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(Some(with_borrowed_ax_element(
                handle,
                crate::actions::post_state::read_element_state,
            )))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_state"))
    }

    fn get_live_actions(&self, handle: &NativeHandle) -> Result<Option<Vec<String>>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(Some(with_borrowed_ax_element(
                handle,
                crate::actions::post_state::read_live_actions,
            )))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_actions"))
    }

    fn get_live_element(&self, handle: &NativeHandle) -> Result<LiveElement, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(with_borrowed_ax_element(
                handle,
                crate::actions::post_state::read_live_element,
            ))
        }
        #[cfg(not(target_os = "macos"))]
        Err(AdapterError::not_supported("get_live_element"))
    }

    fn get_element_bounds(&self, handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        #[cfg(target_os = "macos")]
        {
            Ok(with_borrowed_ax_element(handle, crate::tree::read_bounds))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = handle;
            Err(AdapterError::not_supported("get_element_bounds"))
        }
    }

    fn get_subtree(
        &self,
        handle: &NativeHandle,
        opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        with_borrowed_ax_element(handle, |el| {
            let mut ancestors = FxHashSet::default();
            let context = crate::tree::TreeBuildContext::empty(opts.include_bounds);
            crate::tree::build_subtree(
                el,
                0,
                0,
                opts.max_depth,
                &mut ancestors,
                opts.skeleton,
                &context,
            )
            .ok_or_else(|| {
                AdapterError::new(
                    agent_desktop_core::error::ErrorCode::ElementNotFound,
                    "Element no longer exists in accessibility tree",
                )
                .with_suggestion("Run 'snapshot' to refresh refs, then retry.")
            })
        })
    }
}
