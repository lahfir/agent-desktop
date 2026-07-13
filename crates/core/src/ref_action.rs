use crate::{
    AdapterError, AppError,
    action_request::ActionRequest,
    action_result::ActionResult,
    actionability,
    adapter::{NativeHandle, PlatformAdapter},
    context::CommandContext,
    ref_action_context::RefActionContext,
    refs::RefEntry,
};
use serde_json::json;

const TRACE_CAPTURE_BUDGET_MS: u64 = 1_000;

/// A strictly resolved ref-action target plus the tracing identity for it.
pub(crate) struct ResolvedRefAction<'a> {
    target: RefActionContext<'a>,
    pub(crate) handle: &'a NativeHandle,
}

pub(crate) struct ActionabilityPreflight {
    verified_point: Option<crate::Point>,
    pointer_delivery: actionability::PointerDelivery,
}

impl<'a> ResolvedRefAction<'a> {
    pub(crate) fn new(target: RefActionContext<'a>, handle: &'a NativeHandle) -> Self {
        Self { target, handle }
    }
}

impl<'a> std::ops::Deref for ResolvedRefAction<'a> {
    type Target = RefActionContext<'a>;

    fn deref(&self) -> &Self::Target {
        &self.target
    }
}

pub(crate) fn preflight_resolved(
    target: &ResolvedRefAction<'_>,
    request: &ActionRequest,
    stability: actionability::StabilityExpectation,
) -> Result<ActionabilityPreflight, AppError> {
    check_actionability_with_trace(target, request, stability)
}

pub(crate) fn dispatch_resolved(
    target: RefActionContext<'_>,
    mut request: ActionRequest,
    lease: &crate::InteractionLease,
) -> Result<ActionResult, AppError> {
    let process_instance = target
        .entry
        .process
        .process_instance
        .as_deref()
        .filter(|instance| !instance.is_empty())
        .ok_or_else(|| AdapterError::stale_ref("target process instance is unavailable"))?;
    let expected_process = crate::ProcessIdentity::new(target.entry.process.pid, process_instance);
    let mut handle = target
        .adapter
        .resolve_element_strict(target.entry, target.deadline)
        .map_err(mark_pre_dispatch_resolution_failure)
        .inspect_err(|error| {
            crate::ref_action_wait_support::trace_resolve_error(
                target.context,
                target.ref_id,
                error,
            )
        })?;
    crate::ref_action_wait_support::trace_resolve_ok(target.context, target.ref_id);
    let initial_target = ResolvedRefAction::new(target, &handle);
    let preflight = match stable_preflight(&initial_target, &request) {
        Ok(preflight) => preflight,
        Err(error) => {
            let error = into_adapter_error(error);
            if !crate::ref_action_wait_evidence::should_scroll_after_preflight(&request, &error) {
                return Err(error.into());
            }
            target
                .adapter
                .scroll_into_view(&handle, lease)
                .inspect_err(|error| trace_scroll_error(&target, error))?;
            handle = target
                .adapter
                .resolve_element_strict(target.entry, target.deadline)
                .map_err(mark_pre_dispatch_resolution_failure)?;
            let scrolled_target = ResolvedRefAction::new(target, &handle);
            stable_preflight(&scrolled_target, &request)?
        }
    };
    if matches!(
        preflight.pointer_delivery,
        actionability::PointerDelivery::Semantic
    ) {
        request.policy = crate::InteractionPolicy::headless();
    }
    request = request
        .with_verified_point(preflight.verified_point)
        .with_expected_process(expected_process);
    let final_target = ResolvedRefAction::new(target, &handle);
    final_target.context.trace_lazy(
        "action.dispatch.start",
        || json!({ "ref": final_target.ref_id, "action": request.action.name() }),
    )?;
    let action_name = request.action.name();
    let dispatch_result = final_target
        .adapter
        .execute_action(final_target.handle, request, lease);
    let result = dispatch_result?;
    final_target
        .context
        .trace_lazy(
            "action.dispatch.ok",
            || json!({ "ref": final_target.ref_id, "action": action_name, "result": &result }),
        )
        .map_err(trace_error_after_delivery)?;
    Ok(result)
}

pub(crate) fn mark_pre_dispatch_resolution_failure(error: AdapterError) -> AdapterError {
    match error.disposition {
        crate::DeliverySemantics::Unknown | crate::DeliverySemantics::NotDelivered => {
            error.with_disposition(crate::DeliverySemantics::not_delivered())
        }
        crate::DeliverySemantics::DeliveryUncertain
        | crate::DeliverySemantics::DeliveredUnverified
        | crate::DeliverySemantics::DeliveredVerified => error,
    }
}

pub(crate) fn capture_pre_artifact(
    context: &CommandContext,
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    deadline: crate::Deadline,
) -> crate::trace_artifacts::ArtifactOutcome {
    crate::trace_artifacts::capture_action_screenshot(
        context,
        adapter,
        entry,
        "pre",
        trace_capture_deadline(deadline),
    )
}

pub(crate) fn finish_artifacts(
    context: &CommandContext,
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    ref_id: &str,
    pre: &crate::trace_artifacts::ArtifactOutcome,
    deadline: crate::Deadline,
) {
    let post = crate::trace_artifacts::capture_action_screenshot(
        context,
        adapter,
        entry,
        "post",
        trace_capture_deadline(deadline),
    );
    if let Err(error) = crate::trace_artifacts::emit_action_artifacts(context, ref_id, pre, &post) {
        tracing::warn!(error = %error, ref_id, "action artifact emission failed");
    }
}

fn trace_error_after_delivery(error: AppError) -> AppError {
    crate::context::trace_error_with_disposition(
        error,
        crate::DeliverySemantics::delivered_unverified(),
    )
}

fn trace_capture_deadline(parent: crate::Deadline) -> crate::Deadline {
    let remaining_ms = parent.remaining_ms();
    if remaining_ms == 0 || remaining_ms <= TRACE_CAPTURE_BUDGET_MS {
        return parent;
    }
    crate::Deadline::after(TRACE_CAPTURE_BUDGET_MS).unwrap_or(parent)
}

fn stable_preflight(
    target: &ResolvedRefAction<'_>,
    request: &ActionRequest,
) -> Result<ActionabilityPreflight, AppError> {
    let permissive = check_actionability_with_trace(
        target,
        request,
        actionability::StabilityExpectation::permissive(target.entry.geometry.bounds_hash),
    )?;
    if !actionability::requires_stability(&request.action)
        || matches!(
            permissive.pointer_delivery,
            actionability::PointerDelivery::Semantic
        )
    {
        return Ok(permissive);
    }
    let started = std::time::Instant::now();
    let mut sampler = actionability::stability_sampler::StabilitySampler::new();
    loop {
        let bounds = target
            .adapter
            .get_element_bounds(target.handle, target.deadline)?;
        let elapsed = started.elapsed();
        if sampler.observe(bounds, elapsed) {
            let stability = actionability::StabilityExpectation::strict(
                sampler.bounds(),
                sampler.samples(),
                u64::try_from(sampler.stable_span(elapsed).as_millis()).unwrap_or(u64::MAX),
            );
            return check_actionability_with_trace(target, request, stability);
        }
        let sleep = target
            .deadline
            .remaining_slice(actionability::stability_sampler::STABILITY_SAMPLE_INTERVAL)
            .map_err(|error| error.with_disposition(crate::DeliverySemantics::not_delivered()))?;
        std::thread::sleep(sleep);
    }
}

fn trace_scroll_error(target: &RefActionContext<'_>, error: &AdapterError) {
    let _ = target.context.trace_lazy("ref.scroll_into_view.error", || {
        serde_json::json!({
            "ref": target.ref_id,
            "code": error.code.as_str(),
            "message": error.message,
        })
    });
}

#[cfg(test)]
pub(crate) fn execute_resolved(
    target: ResolvedRefAction<'_>,
    request: ActionRequest,
    lease: &crate::InteractionLease,
) -> Result<ActionResult, AppError> {
    let pre = capture_pre_artifact(
        target.context,
        target.adapter,
        target.entry,
        target.deadline,
    );
    let stability =
        actionability::StabilityExpectation::permissive(target.entry.geometry.bounds_hash);
    preflight_resolved(&target, &request, stability)?;
    let context = target.context;
    let adapter = target.adapter;
    let entry = target.entry;
    let ref_id = target.ref_id;
    let deadline = target.deadline;
    let result = dispatch_resolved(target.target, request, lease);
    finish_artifacts(context, adapter, entry, ref_id, &pre, deadline);
    result
}

fn check_actionability_with_trace(
    target: &ResolvedRefAction<'_>,
    request: &ActionRequest,
    stability: actionability::StabilityExpectation,
) -> Result<ActionabilityPreflight, AppError> {
    target.context.trace_lazy(
        "actionability.check.start",
        || json!({ "ref": target.ref_id, "action": request.action.name() }),
    )?;
    let report = actionability::check_live_with_stability(
        target.entry,
        target.handle,
        target.adapter,
        request,
        stability,
        target.deadline,
    )
    .inspect_err(|err| {
        let _ = target.context.trace_lazy("actionability.check.error", || {
            json!({
                "ref": target.ref_id,
                "action": request.action.name(),
                "code": err.code.as_str(),
                "message": err.message.clone(),
                "details": err.details.clone()
            })
        });
    })?;
    target.context.trace_lazy("actionability.check.ok", || {
        json!({ "ref": target.ref_id, "action": request.action.name(), "report": json!(report) })
    })?;
    Ok(ActionabilityPreflight {
        verified_point: report.verified_point,
        pointer_delivery: report.pointer_delivery,
    })
}

/// Builds a stable, non-sensitive trace label from a `RefEntry`. The label
/// is derived from role and path indices only — no content fields — so it is
/// safe to emit in the `"ref"` trace key without redaction risk. Path indices
/// are deterministic within a snapshot but carry no secret information.
fn ref_label_from_entry(entry: &RefEntry) -> String {
    if entry.scope.path.is_empty() {
        return format!("<{}>", entry.identity.role);
    }
    let indices: Vec<String> = entry.scope.path.iter().map(|i| i.to_string()).collect();
    format!("<{}/{}>", entry.identity.role, indices.join("/"))
}

/// Executes a pre-resolved ref-action entry using the provided `context` for
/// session identity and trace emission. Prefer this over `execute_entry` when
/// a real `CommandContext` is available (e.g. from `AdAdapter::command_context`
/// in the FFI layer), so that trace events carry the correct session id.
///
/// Trace records use a role/path-derived label for the `"ref"` field so that
/// FFI call-site events are distinguishable in multi-element trace logs. The
/// label never includes content fields (name, value, text) that are subject to
/// redaction.
pub fn execute_entry_with_context(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    request: ActionRequest,
    context: &CommandContext,
) -> Result<ActionResult, AdapterError> {
    let label = ref_label_from_entry(entry);
    crate::ref_action_wait::execute_with_auto_wait(
        crate::ref_action_wait_context::RefActionWaitContext {
            adapter,
            entry,
            ref_id: &label,
            context,
        },
        request,
        dispatch_resolved,
    )
}

/// Executes a pre-resolved ref-action entry with a default (no-session,
/// no-trace) `CommandContext`. Existing callers outside the FFI layer that do
/// not have a live session context continue to use this entry point unchanged.
pub fn execute_entry(
    adapter: &dyn PlatformAdapter,
    entry: &RefEntry,
    request: ActionRequest,
) -> Result<ActionResult, AdapterError> {
    execute_entry_with_context(adapter, entry, request, &CommandContext::default())
}

pub(crate) fn into_adapter_error(err: AppError) -> AdapterError {
    match err {
        AppError::Adapter(err) => err,
        other => AdapterError::internal(other.to_string()),
    }
}

#[cfg(test)]
#[path = "ref_action_tests.rs"]
mod tests;
