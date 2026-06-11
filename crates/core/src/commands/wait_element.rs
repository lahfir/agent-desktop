use crate::{
    adapter::PlatformAdapter,
    commands::{wait_latest_ref_cache::LatestRefCache, wait_predicate, wait_timeout},
    context::CommandContext,
    error::{AppError, ErrorCode},
    refs_store::RefStore,
};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

/// Per-attempt cap on ref resolution inside a wait loop, so a slow resolve
/// cannot consume the whole wait budget on the first poll; the predicate is
/// re-checked every attempt across the full timeout.
const WAIT_RESOLVE_ATTEMPT: Duration = Duration::from_millis(750);

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
    let timeout = Duration::from_millis(timeout_ms);
    let store = RefStore::for_session(context.session_id())?;
    let fixed_refmap = match snapshot_id.as_deref() {
        Some(id) => Some(store.load_snapshot(id)?),
        None => None,
    };
    let mut latest_cache = if fixed_refmap.is_none() {
        Some(LatestRefCache::new(&store)?)
    } else {
        None
    };

    if fixed_refmap
        .as_ref()
        .is_some_and(|refmap| refmap.get(&ref_id).is_none())
    {
        return Err(AppError::invalid_input_with_suggestion(
            format!("Ref {ref_id} is not present in the requested snapshot"),
            "Use a ref returned by that snapshot_id, or omit --snapshot to wait against the latest refmap.",
        ));
    }

    let mut last_observed = json!(null);
    loop {
        let entry = fixed_refmap
            .as_ref()
            .and_then(|r| r.get(&ref_id).cloned())
            .or_else(|| latest_cache.as_ref().and_then(|c| c.entry(&ref_id)));
        if let Some(entry) = entry {
            let remaining = timeout.saturating_sub(start.elapsed());
            if remaining.is_zero() {
                return wait_timeout::element(ref_id, predicate, timeout_ms, last_observed);
            }
            let attempt = remaining.min(WAIT_RESOLVE_ATTEMPT);
            match adapter.resolve_element_strict_with_timeout(&entry, attempt) {
                Ok(handle) => {
                    let observed = wait_predicate::observe(&entry, &handle, &predicate, adapter);
                    let _ = adapter.release_handle(&handle);
                    last_observed = observed.map_err(AppError::Adapter)?;
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
                Err(err) if is_retryable_wait_resolution_error(&err.code) => {
                    last_observed = json!({
                        "error": err.code.as_str(),
                        "message": err.message
                    });
                    if fixed_refmap.is_none() {
                        if let Some(cache) = latest_cache.as_mut() {
                            cache.refresh_if_due();
                        }
                    }
                }
                Err(err) => return Err(AppError::Adapter(err)),
            }
        } else if let Some(cache) = latest_cache.as_mut() {
            cache.refresh_if_due();
        }

        let remaining = timeout.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            return wait_timeout::element(ref_id, predicate, timeout_ms, last_observed);
        }
        std::thread::sleep(remaining.min(Duration::from_millis(100)));
    }
}

fn is_retryable_wait_resolution_error(code: &ErrorCode) -> bool {
    matches!(
        code,
        ErrorCode::StaleRef
            | ErrorCode::ElementNotFound
            | ErrorCode::AmbiguousTarget
            | ErrorCode::Timeout
    )
}
