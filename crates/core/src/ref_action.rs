use crate::{
    action::{ActionRequest, ActionResult},
    actionability,
    adapter::PlatformAdapter,
    error::AdapterError,
    refs::RefEntry,
};

pub fn execute_entry(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    let handle = adapter.resolve_element_strict(entry)?;
    let result = actionability::check_live(entry, &handle, adapter, &request)
        .and_then(|_| adapter.execute_action(&handle, request));
    let release = adapter.release_handle(&handle);
    match (result, release) {
        (Ok(result), Ok(())) => Ok(result),
        (Ok(_), Err(err)) | (Err(err), _) => Err(err),
    }
}
