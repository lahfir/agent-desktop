use crate::{
    AdapterError, InteractionLease, action_request::ActionRequest, action_result::ActionResult,
    native_handle::NativeHandle,
};

pub trait ActionOps: Send + Sync {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("execute_action"))
    }

    fn scroll_into_view(
        &self,
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<(), AdapterError> {
        let _ = (handle, lease);
        Err(AdapterError::not_supported("scroll_into_view"))
    }
}
