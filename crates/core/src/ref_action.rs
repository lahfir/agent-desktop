use crate::{
    action_request::ActionRequest,
    action_result::ActionResult,
    actionability,
    adapter::{NativeHandle, PlatformAdapter},
    context::CommandContext,
    error::{AdapterError, AppError},
    refs::RefEntry,
    resolved_element::ResolvedElement,
};
use serde_json::json;

/// A strictly resolved ref-action target plus the tracing identity for it.
/// Both the CLI command path and the FFI entry path execute through
/// [`execute_resolved`], so actionability, dispatch, and trace semantics
/// live in exactly one place.
pub(crate) struct ResolvedRefAction<'a> {
    pub(crate) adapter: &'a dyn PlatformAdapter,
    pub(crate) entry: &'a RefEntry,
    pub(crate) handle: &'a NativeHandle,
    pub(crate) ref_id: &'a str,
    pub(crate) context: &'a CommandContext,
}

pub(crate) fn execute_resolved(
    target: ResolvedRefAction<'_>,
    request: ActionRequest,
) -> Result<ActionResult, AppError> {
    check_actionability_with_trace(&target, &request)?;
    target.context.trace_lazy(
        "action.dispatch.start",
        || json!({ "ref": target.ref_id, "action": request.action.name() }),
    )?;
    let action_name = request.action.name();
    let result = target.adapter.execute_action(target.handle, request)?;
    let _ = target.context.trace_lazy(
        "action.dispatch.ok",
        || json!({ "ref": target.ref_id, "action": action_name, "result": &result }),
    );
    Ok(result)
}

fn check_actionability_with_trace(
    target: &ResolvedRefAction<'_>,
    request: &ActionRequest,
) -> Result<(), AppError> {
    target.context.trace_lazy(
        "actionability.check.start",
        || json!({ "ref": target.ref_id, "action": request.action.name() }),
    )?;
    actionability::check_live(target.entry, target.handle, target.adapter, request).inspect_err(
        |err| {
            let _ = target.context.trace_lazy("actionability.check.error", || {
                json!({
                    "ref": target.ref_id,
                    "action": request.action.name(),
                    "code": err.code.as_str(),
                    "message": err.message.clone(),
                    "details": err.details.clone()
                })
            });
        },
    )?;
    target.context.trace_lazy(
        "actionability.check.ok",
        || json!({ "ref": target.ref_id, "action": request.action.name() }),
    )?;
    Ok(())
}

pub fn execute_entry(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    let handle = adapter.resolve_element_strict(entry)?;
    let handle = ResolvedElement::new(adapter, handle);
    let context = CommandContext::default();
    let result = execute_resolved(
        ResolvedRefAction {
            adapter,
            entry,
            handle: handle.handle(),
            ref_id: "<ffi>",
            context: &context,
        },
        request,
    );
    result.map_err(into_adapter_error)
}

fn into_adapter_error(err: AppError) -> AdapterError {
    match err {
        AppError::Adapter(err) => err,
        other => AdapterError::internal(other.to_string()),
    }
}

#[cfg(test)]
#[path = "ref_action_tests.rs"]
mod tests;
