use crate::{
    action::{ActionRequest, ActionResult},
    actionability::{self, ActionabilityReport},
    adapter::{NativeHandle, PlatformAdapter},
    error::AdapterError,
    refs::RefEntry,
};

pub(crate) fn check_resolved(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    handle: &NativeHandle,
    request: &ActionRequest,
) -> Result<ActionabilityReport, AdapterError> {
    actionability::check_live(entry, handle, adapter, request)
}

pub fn execute_entry(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    let handle = adapter.resolve_element_strict(entry)?;
    let result = check_resolved(adapter, entry, &handle, &request)
        .and_then(|_| adapter.execute_action(&handle, request));
    let release = adapter.release_handle(&handle);
    match (result, release) {
        (Ok(result), Ok(())) => Ok(result),
        (Ok(_), Err(err)) | (Err(err), _) => Err(err),
    }
}
