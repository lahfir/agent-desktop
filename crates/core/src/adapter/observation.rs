use crate::{
    element_state::ElementState,
    error::{AdapterError, ErrorCode},
    live_element::LiveElement,
    native_handle::NativeHandle,
    node::{AccessibilityNode, AppInfo, Rect, SurfaceInfo, WindowInfo},
    refs::RefEntry,
    tree_options::TreeOptions,
    window_filter::WindowFilter,
};

pub(crate) fn optional_live_read<T>(
    result: Result<Option<T>, AdapterError>,
) -> Result<Option<T>, AdapterError> {
    match result {
        Ok(value) => Ok(value),
        Err(err) if is_live_read_unsupported(&err) => Ok(None),
        Err(err) => Err(err),
    }
}

fn is_live_read_unsupported(err: &AdapterError) -> bool {
    matches!(
        err.code,
        ErrorCode::PlatformNotSupported | ErrorCode::ActionNotSupported
    )
}

pub trait ObservationOps: Send + Sync {
    fn list_windows(&self, _filter: &WindowFilter) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_windows"))
    }

    fn list_apps(&self) -> Result<Vec<AppInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_apps"))
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        Err(AdapterError::not_supported("get_tree"))
    }

    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::not_supported("resolve_element_strict"))
    }

    /// Resolves an element under a caller deadline. Defaults to delegating
    /// to [`ObservationOps::resolve_element_strict`], ignoring the timeout,
    /// so adapters that implement only the un-timed variant still support
    /// `wait --element`. Override to honor the remaining budget.
    fn resolve_element_strict_with_timeout(
        &self,
        entry: &RefEntry,
        timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        tracing::trace!(
            ?timeout,
            "resolve_element_strict_with_timeout: default impl does not enforce the deadline; override to honor it"
        );
        self.resolve_element_strict(entry)
    }

    fn get_subtree(
        &self,
        _handle: &NativeHandle,
        _opts: &TreeOptions,
    ) -> Result<AccessibilityNode, AdapterError> {
        Err(AdapterError::not_supported("get_subtree"))
    }

    fn list_surfaces(&self, _pid: i32) -> Result<Vec<SurfaceInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_surfaces"))
    }

    fn get_live_value(&self, _handle: &NativeHandle) -> Result<Option<String>, AdapterError> {
        Err(AdapterError::not_supported("get_live_value"))
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        Err(AdapterError::not_supported("get_live_state"))
    }

    fn get_live_actions(
        &self,
        _handle: &NativeHandle,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        Err(AdapterError::not_supported("get_live_actions"))
    }

    fn get_live_element(&self, handle: &NativeHandle) -> Result<LiveElement, AdapterError> {
        let live = LiveElement {
            state: optional_live_read(self.get_live_state(handle))?,
            bounds: optional_live_read(self.get_element_bounds(handle))?,
            available_actions: optional_live_read(self.get_live_actions(handle))?,
        };
        if live.state.is_none() && live.bounds.is_none() && live.available_actions.is_none() {
            return Err(AdapterError::not_supported("get_live_element"));
        }
        Ok(live)
    }

    fn get_element_bounds(&self, _handle: &NativeHandle) -> Result<Option<Rect>, AdapterError> {
        Err(AdapterError::not_supported("get_element_bounds"))
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        point: crate::action::Point,
    ) -> Result<crate::hit_test::HitTestResult, AdapterError> {
        let _ = (handle, point);
        Err(AdapterError::not_supported("hit_test"))
    }
}
