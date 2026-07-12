use crate::{
    AppError,
    action_request::ActionRequest,
    adapter::{PlatformAdapter, TreeOptions},
    commands::{wait_selector, wait_selector::WaitSelectorInput},
    context::CommandContext,
    ref_action_wait_context::RefActionWaitContext,
    ref_resolve_deadline::resolve_within_deadline,
    refs::RefEntry,
    refs_store::RefStore,
    resolve_attempt_outcome::ResolveAttemptOutcome,
    window_lookup,
};
use serde_json::{Value, json};

pub(crate) use crate::app_lookup::{process_identity, resolve_app, revalidate_app_for_mutation};

pub use super::window_target::AppArgs;
pub(crate) use super::window_target::{
    resolve_window_for_app, revalidate_window_for_mutation, window_op_command,
};

pub struct RefArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub timeout_ms: Option<u64>,
}

pub(crate) fn acquire_interaction_lease(
    adapter: &dyn PlatformAdapter,
) -> Result<crate::InteractionLease, AppError> {
    Ok(adapter.acquire_interaction_lease(crate::Deadline::standard()?)?)
}

pub fn normalize_action_timeout_ms(raw: u64) -> Option<u64> {
    if raw == 0 { None } else { Some(raw) }
}

pub(crate) fn resolve_ref_with_context(
    ref_id: &str,
    snapshot_id: Option<&str>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<(RefEntry, crate::adapter::NativeHandle), AppError> {
    resolve_ref_within_deadline(
        ref_id,
        snapshot_id,
        crate::Deadline::standard()?,
        adapter,
        context,
    )
}

/// Resolves a ref to a live element handle, capping the strict resolve to
/// `deadline` when supplied (the `hover`/`drag` wait path) or resolving
/// uncapped otherwise (`get`/`is`). Delegates entry loading and its
/// `ref.resolve.start/entry/error` tracing to [`load_ref_entry`], then adds
/// handle resolution and the `ref.resolve.ok` event, so budgeted and
/// single-shot resolution trace identically.
fn resolve_ref_within_deadline(
    ref_id: &str,
    snapshot_id: Option<&str>,
    deadline: crate::Deadline,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<(RefEntry, crate::adapter::NativeHandle), AppError> {
    let entry = load_ref_entry(ref_id, snapshot_id, context)?;
    let handle = resolve_handle_within_deadline(adapter, &entry, deadline).inspect_err(|err| {
        let _ = context.trace_lazy("ref.resolve.error", || {
            json!({
                "ref": ref_id,
                "snapshot_id": snapshot_id,
                "code": err.code.as_str(),
                "message": err.message.clone(),
                "details": err.details.clone()
            })
        });
    })?;
    tracing::debug!("resolve: {} resolved successfully", ref_id);
    context.trace_lazy("ref.resolve.ok", || json!({ "ref": ref_id }))?;
    Ok((entry, handle))
}

/// Performs the strict resolve for [`resolve_ref_within_deadline`], capping the
/// attempt to `deadline` when one is supplied and surfacing an exhausted budget
/// as a `TIMEOUT`, or resolving uncapped when it is not.
pub(crate) fn resolve_handle_within_deadline(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: crate::Deadline,
) -> Result<crate::adapter::NativeHandle, crate::AdapterError> {
    match resolve_within_deadline(adapter, entry, deadline) {
        ResolveAttemptOutcome::Resolved(handle) => Ok(handle),
        ResolveAttemptOutcome::Failed(err) => Err(err),
        ResolveAttemptOutcome::DeadlinePassed => Err(crate::AdapterError::timeout(
            "Target did not resolve within the wait budget",
        )),
    }
}

pub(crate) fn execute_ref_action_with_context(
    args: RefArgs,
    adapter: &dyn PlatformAdapter,
    request: ActionRequest,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let request = request.with_timeout_ms(args.timeout_ms);
    validate_post_action_wait(context)?;
    let entry = load_ref_entry(&args.ref_id, args.snapshot_id.as_deref(), context)?;
    let (result, lease, pre, deadline, lease_started) =
        crate::ref_action_wait::execute_with_auto_wait_and_lease(
            RefActionWaitContext {
                adapter,
                entry: &entry,
                ref_id: &args.ref_id,
                context,
            },
            request,
            crate::ref_action::dispatch_resolved,
        )
        .map_err(AppError::Adapter)?;
    let value = serde_json::to_value(result).map_err(|error| {
        post_delivery_error(AppError::Json(error), json!({ "action": "delivered" }))
    })?;
    let mut outcome = apply_post_action_wait(value, Some(&entry), adapter, context, &lease);
    let lease_hold_ms = u64::try_from(lease_started.elapsed().as_millis()).unwrap_or(u64::MAX);
    update_lease_hold_ms(&mut outcome, lease_hold_ms);
    drop(lease);
    crate::ref_action::finish_artifacts(context, adapter, &entry, &args.ref_id, &pre, deadline);
    outcome
}

pub(crate) fn probe_app_name(adapter: &dyn PlatformAdapter, entry: &RefEntry) -> Option<String> {
    if entry.source.source_app.is_some() {
        return entry.source.source_app.clone();
    }
    let identity = crate::ProcessIdentity::new(
        entry.process.pid,
        entry.process.process_instance.as_deref()?,
    );
    window_lookup::find_window_for_process(identity, adapter, crate::Deadline::standard().ok()?)
        .ok()
        .map(|window| window.app)
}

pub(crate) fn apply_post_action_wait(
    result: Value,
    entry: Option<&RefEntry>,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    _lease: &crate::InteractionLease,
) -> Result<Value, AppError> {
    let Some(wait) = context.wait_selector() else {
        return Ok(result);
    };
    match wait_selector::execute(
        WaitSelectorInput {
            query_raw: wait.query_raw.clone(),
            gone: wait.gone,
            app: entry.and_then(|entry| probe_app_name(adapter, entry)),
            window_id: entry.and_then(|entry| entry.source.source_window_id.clone()),
            opts: TreeOptions::default(),
            timeout_ms: wait.timeout_ms,
        },
        adapter,
        context,
    ) {
        Ok(mut snapshot) => {
            if let Some(body) = snapshot.as_object_mut() {
                body.insert("after_action".into(), result);
            }
            Ok(snapshot)
        }
        Err(AppError::Adapter(mut adapter_err)) => {
            let mut details = adapter_err.details.take().unwrap_or_else(|| json!({}));
            if let Some(obj) = details.as_object_mut() {
                obj.insert("after_action".into(), result);
            }
            Err(AppError::Adapter(
                adapter_err
                    .with_details(details)
                    .with_disposition(crate::DeliverySemantics::delivered_unverified()),
            ))
        }
        Err(err) => Err(post_delivery_error(err, result)),
    }
}

fn update_lease_hold_ms(result: &mut Result<Value, AppError>, lease_hold_ms: u64) {
    fn update(value: &mut Value, lease_hold_ms: u64) {
        if let Some(object) = value.as_object_mut() {
            if let Some(auto_wait) = object.get_mut("auto_wait").and_then(Value::as_object_mut) {
                auto_wait.insert("lease_hold_ms".into(), json!(lease_hold_ms));
            }
            for value in object.values_mut() {
                update(value, lease_hold_ms);
            }
        } else if let Some(values) = value.as_array_mut() {
            for value in values {
                update(value, lease_hold_ms);
            }
        }
    }

    match result {
        Ok(value) => update(value, lease_hold_ms),
        Err(AppError::Adapter(error)) => {
            if let Some(details) = &mut error.details {
                update(details, lease_hold_ms);
            }
        }
        Err(_) => {}
    }
}

pub(crate) fn validate_post_action_wait(context: &CommandContext) -> Result<(), AppError> {
    let Some(wait) = context.wait_selector() else {
        return Ok(());
    };
    crate::commands::query::validate_selector(&wait.query_raw)?;
    crate::Deadline::after(wait.timeout_ms)?;
    Ok(())
}

fn post_delivery_error(error: AppError, result: Value) -> AppError {
    let mut adapter_error = match error {
        AppError::Adapter(error) => error,
        other => crate::AdapterError::internal(other.to_string()),
    };
    let mut details = adapter_error.details.take().unwrap_or_else(|| json!({}));
    if let Some(object) = details.as_object_mut() {
        object.insert("after_action".into(), result);
    }
    AppError::Adapter(
        adapter_error
            .with_details(details)
            .with_disposition(crate::DeliverySemantics::delivered_unverified()),
    )
}

#[cfg(test)]
pub(crate) fn execute_ref_action_result_with_context(
    ref_id: &str,
    snapshot_id: Option<&str>,
    adapter: &dyn PlatformAdapter,
    request: ActionRequest,
    context: &CommandContext,
) -> Result<(RefEntry, crate::ActionResult), AppError> {
    let entry = load_ref_entry(ref_id, snapshot_id, context)?;
    let result = crate::ref_action_wait::execute_with_auto_wait(
        RefActionWaitContext {
            adapter,
            entry: &entry,
            ref_id,
            context,
        },
        request,
        crate::ref_action::dispatch_resolved,
    )
    .map_err(AppError::Adapter)?;
    Ok((entry, result))
}

/// Shared owner of ref-entry loading and its `ref.resolve.start/entry/error`
/// tracing, used by both the ref-action path
/// ([`execute_ref_action_result_with_context`]) and, via
/// [`resolve_ref_within_deadline`], the pointer/get/is resolve path, so a
/// stale ref emits identical telemetry regardless of caller.
/// [`resolve_ref_within_deadline`] builds handle resolution and the
/// `ref.resolve.ok` event on top of the entry this returns.
pub(crate) fn load_ref_entry(
    ref_id: &str,
    snapshot_id: Option<&str>,
    context: &CommandContext,
) -> Result<RefEntry, AppError> {
    let (resolved_snapshot_id, local_ref) =
        crate::ref_token::resolve_ref_target(ref_id, snapshot_id)?;
    let store = RefStore::for_session(context.session_id())?;
    context.trace_lazy(
        "ref.resolve.start",
        || json!({ "ref": ref_id, "snapshot_id": resolved_snapshot_id }),
    )?;
    let refmap = store
        .load_snapshot(&resolved_snapshot_id)
        .inspect_err(|e| {
            tracing::debug!("refmap load failed: {e}");
            let _ = context.trace_lazy("ref.resolve.error", || {
                json!({
                    "ref": ref_id,
                    "snapshot_id": resolved_snapshot_id,
                    "code": e.code(),
                    "message": e.to_string()
                })
            });
        })?;
    let entry = match refmap.get(&local_ref) {
        Some(entry) => entry.clone(),
        None => {
            context.trace_lazy("ref.resolve.error", || {
                json!({
                    "ref": ref_id,
                    "snapshot_id": resolved_snapshot_id,
                    "code": "STALE_REF",
                    "message": "ref not found in current RefMap"
                })
            })?;
            return Err(AppError::stale_ref(ref_id));
        }
    };
    tracing::debug!(
        "resolve: {} -> pid={} role={} name_chars={:?}",
        ref_id,
        entry.process.pid,
        entry.identity.role,
        entry
            .identity
            .name
            .as_deref()
            .map(|name| name.chars().count())
    );
    context.trace_lazy("ref.resolve.entry", || {
        json!({
            "ref": ref_id,
            "pid": entry.process.pid,
            "role": entry.identity.role,
            "name": entry.identity.name
        })
    })?;
    Ok(entry)
}

#[cfg(test)]
#[path = "helpers_test_support.rs"]
mod test_support;

#[cfg(test)]
#[path = "helpers_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "helpers_ref_action_dispatch_tests.rs"]
mod ref_action_dispatch_tests;

#[cfg(test)]
#[path = "helpers_ref_action_wait_tests.rs"]
mod ref_action_wait_tests;
