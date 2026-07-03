use agent_desktop_core::{
    action_request::ActionRequest,
    action_result::ActionResult,
    adapter::{ActionOps, NativeHandle},
    error::AdapterError,
};

use crate::adapter::{MacOSAdapter, with_borrowed_ax_element};

impl ActionOps for MacOSAdapter {
    fn execute_action(
        &self,
        handle: &NativeHandle,
        request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        execute_action_impl(handle, request)
    }

    fn release_handle(&self, handle: &NativeHandle) -> Result<(), AdapterError> {
        let raw = handle.as_raw();
        if raw.is_null() {
            return Ok(());
        }
        unsafe {
            core_foundation::base::CFRelease(raw as core_foundation::base::CFTypeRef);
        }
        Ok(())
    }

    fn scroll_into_view(&self, handle: &NativeHandle) -> Result<(), AdapterError> {
        with_borrowed_ax_element(
            handle,
            crate::actions::scroll_into_view::scroll_into_view_impl,
        )
    }
}

fn execute_action_impl(
    handle: &NativeHandle,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    with_borrowed_ax_element(handle, |el| crate::actions::perform_action(el, &request))
}
