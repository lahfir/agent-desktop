use crate::{
    action_request::ActionRequest, action_result::ActionResult, actionability,
    adapter::PlatformAdapter, error::AdapterError, refs::RefEntry,
};

pub fn execute_entry(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    let handle = adapter.resolve_element_strict(entry)?;
    let result = actionability::check_live(entry, &handle, adapter, &request)
        .and_then(|_| adapter.execute_action(&handle, request));
    let _ = adapter.release_handle(&handle);
    result
}

#[cfg(test)]
#[path = "ref_action_tests.rs"]
mod tests;
