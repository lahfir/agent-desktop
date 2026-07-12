use crate::{
    AccessibilityNode, AdapterError, AppInfo, Deadline, ErrorCode, Rect, SurfaceInfo, WindowInfo,
    element_state::ElementState,
    live_element::LiveElement,
    live_locator::{ObservationRequest, ObservationRoot, ObservedTree},
    native_handle::NativeHandle,
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
    fn observe_tree(
        &self,
        _root: ObservationRoot<'_>,
        _request: &ObservationRequest,
    ) -> Result<ObservedTree, AdapterError> {
        Err(AdapterError::not_supported("observe_tree"))
    }

    fn list_windows(
        &self,
        _filter: &WindowFilter,
        _deadline: Deadline,
    ) -> Result<Vec<WindowInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_windows"))
    }

    fn list_apps(&self, _deadline: Deadline) -> Result<Vec<AppInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_apps"))
    }

    fn list_apps_scoped(
        &self,
        name: &str,
        bundle_id: Option<&str>,
        deadline: Deadline,
    ) -> Result<Vec<AppInfo>, AdapterError> {
        Ok(self
            .list_apps(deadline)?
            .into_iter()
            .filter(|app| {
                app.name.eq_ignore_ascii_case(name)
                    && bundle_id.is_none_or(|expected| {
                        app.bundle_id
                            .as_deref()
                            .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
                    })
            })
            .collect())
    }

    fn get_tree(
        &self,
        _win: &WindowInfo,
        _opts: &TreeOptions,
        _deadline: Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        Err(AdapterError::not_supported("get_tree"))
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::not_supported("resolve_element_strict"))
    }

    fn resolve_locator_anchor(
        &self,
        _entry: &RefEntry,
        _deadline: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Err(AdapterError::not_supported("resolve_locator_anchor"))
    }

    fn get_subtree(
        &self,
        _handle: &NativeHandle,
        _opts: &TreeOptions,
        _deadline: Deadline,
    ) -> Result<AccessibilityNode, AdapterError> {
        Err(AdapterError::not_supported("get_subtree"))
    }

    fn list_surfaces(
        &self,
        _process: crate::ProcessIdentity,
        _deadline: Deadline,
    ) -> Result<Vec<SurfaceInfo>, AdapterError> {
        Err(AdapterError::not_supported("list_surfaces"))
    }

    fn get_live_value(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<String>, AdapterError> {
        Err(AdapterError::not_supported("get_live_value"))
    }

    fn get_live_state(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<ElementState>, AdapterError> {
        Err(AdapterError::not_supported("get_live_state"))
    }

    fn get_live_actions(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Vec<String>>, AdapterError> {
        Err(AdapterError::not_supported("get_live_actions"))
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<LiveElement, AdapterError> {
        Err(AdapterError::not_supported("get_live_element"))
    }

    fn get_element_bounds(
        &self,
        _handle: &NativeHandle,
        _deadline: Deadline,
    ) -> Result<Option<Rect>, AdapterError> {
        Err(AdapterError::not_supported("get_element_bounds"))
    }

    fn hit_test(
        &self,
        handle: &NativeHandle,
        point: crate::Point,
        _deadline: Deadline,
    ) -> Result<crate::hit_test::HitTestResult, AdapterError> {
        let _ = (handle, point);
        Err(AdapterError::not_supported("hit_test"))
    }
}

#[cfg(test)]
#[path = "observation_tests.rs"]
mod tests;
