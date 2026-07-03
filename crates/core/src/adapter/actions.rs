use crate::{
    action_request::ActionRequest, action_result::ActionResult, error::AdapterError,
    native_handle::NativeHandle,
};

pub trait ActionOps: Send + Sync {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("execute_action"))
    }

    /// Releases a platform handle that an implementation took ownership of during resolve.
    /// Adapter methods that receive `&NativeHandle` borrow it only; they must not consume
    /// or release it. The default no-op is correct for adapters whose handles are owned
    /// or freed elsewhere.
    fn release_handle(&self, _handle: &NativeHandle) -> Result<(), AdapterError> {
        Ok(())
    }
}
