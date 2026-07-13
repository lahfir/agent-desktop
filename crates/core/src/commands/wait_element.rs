use crate::{
    AppError,
    adapter::PlatformAdapter,
    commands::{wait_predicate, wait_timeout},
    context::CommandContext,
    ref_resolve_deadline::{POLL_INTERVAL, resolve_within_deadline},
    refs_store::RefStore,
    resolve_attempt_outcome::ResolveAttemptOutcome,
};
use serde_json::{Value, json};
use std::time::Instant;

pub(crate) struct ElementWaitInput {
    pub(crate) ref_id: String,
    pub(crate) snapshot_id: Option<String>,
    pub(crate) predicate: wait_predicate::ElementPredicate,
    pub(crate) timeout_ms: u64,
}

pub(crate) fn wait_for_element(
    input: ElementWaitInput,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let ElementWaitInput {
        ref_id,
        snapshot_id,
        predicate,
        timeout_ms,
    } = input;
    let start = Instant::now();
    let deadline = crate::Deadline::at(start, timeout_ms)?;
    let (resolved_snapshot_id, local_ref) =
        crate::ref_token::resolve_ref_target(&ref_id, snapshot_id.as_deref())?;
    let store = RefStore::for_session(context.session_id())?;
    let refmap = store.load_snapshot(&resolved_snapshot_id)?;
    let entry = refmap.get(&local_ref).cloned().ok_or_else(|| {
        AppError::invalid_input_with_suggestion(
            format!("Ref {ref_id} is not present in the requested snapshot"),
            "Use a snapshot-qualified ref returned by that snapshot, or pair a legacy @eN ref with its snapshot_id.",
        )
    })?;

    let mut last_observed = json!(null);
    let mut expected_bounds_hash = None;
    loop {
        match resolve_within_deadline(adapter, &entry, deadline) {
            ResolveAttemptOutcome::DeadlinePassed => {
                return wait_timeout::element(ref_id, predicate, timeout_ms, last_observed);
            }
            ResolveAttemptOutcome::Resolved(handle) => {
                match wait_predicate::observe(
                    &entry,
                    &handle,
                    &predicate,
                    adapter,
                    deadline,
                    crate::actionability::StabilityExpectation::strict_hash(expected_bounds_hash),
                ) {
                    Ok(observed) => {
                        last_observed = observed;
                        if let Some(observed) = last_observed
                            .get("observed_bounds_hash")
                            .and_then(Value::as_u64)
                        {
                            expected_bounds_hash = Some(observed);
                        }
                        if wait_predicate::satisfied(&predicate, &last_observed) {
                            let elapsed = start.elapsed().as_millis();
                            return Ok(json!({
                                "found": true,
                                "ref": ref_id,
                                "predicate": predicate.name(),
                                "observed": last_observed,
                                "elapsed_ms": elapsed
                            }));
                        }
                    }
                    Err(err) if is_retryable_wait_error(&err) => {
                        last_observed = json!({
                            "error": err.code.as_str(),
                            "message": err.message,
                            "details": err.details
                        });
                    }
                    Err(err) => return Err(AppError::Adapter(err)),
                }
            }
            ResolveAttemptOutcome::Failed(err) if is_retryable_wait_error(&err) => {
                last_observed = json!({
                    "error": err.code.as_str(),
                    "message": err.message
                });
            }
            ResolveAttemptOutcome::Failed(err) => return Err(AppError::Adapter(err)),
        }

        let remaining = deadline.remaining();
        if remaining.is_zero() {
            return wait_timeout::element(ref_id, predicate, timeout_ms, last_observed);
        }
        std::thread::sleep(remaining.min(POLL_INTERVAL));
    }
}

fn is_retryable_wait_error(error: &crate::AdapterError) -> bool {
    error.is_explicitly_retryable()
}
