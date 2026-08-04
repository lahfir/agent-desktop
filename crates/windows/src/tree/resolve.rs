use agent_desktop_core::{AdapterError, Deadline, ErrorCode, NativeHandle, RefEntry};

#[cfg(target_os = "windows")]
use super::element::UIAElement;
#[cfg(target_os = "windows")]
use super::resolve_match::{CandidateOutcome, ambiguous_target_error};
use super::resolve_match::stale_ref_error;
#[cfg(target_os = "windows")]
use super::resolve_search::{
    MAX_RESOLVE_DEPTH, can_use_path_fast_path, element_at_path, geometry_matches,
    identity_unknown_error, search_under,
};
#[cfg(target_os = "windows")]
use super::walker::{DEFAULT_MAX_SIBLINGS, TreeSource, WalkBudget};
#[cfg(target_os = "windows")]
use super::walker_source::UiaTreeSource;

/// Resolves a stored ref to its live element, fail-closed and three-state.
///
/// The search descends from the stored window's root to a resolve-scoped
/// depth, reading each candidate with the same composition the walk uses
/// (`UiaTreeSource::evidence`), gates on role, and runs core's composed
/// identity rule (`resolve_match::candidate_outcome`). Then:
///
/// - zero candidates and every decision was readable -> `STALE_REF`, settled
/// - zero candidates but one was **unreadable** -> incomplete-and-retryable
///   (`AppUnresponsive`, the three-state discipline: an `Unknown` verdict is
///   never a `NoMatch`)
/// - two or more candidates that all match -> `AMBIGUOUS_TARGET`
/// - exactly one -> a `NativeHandle` wrapping the live element
///
/// Anything short of an exact match fails closed rather than guessing,
/// because A7-3 measured Explorer re-resolving 29 of 29 `AutomationId` keys
/// with 5 landing on a different element - the silent-wrong-target shape
/// strictness exists to prevent.
///
/// The whole resolution runs through a `deadline`-bounded retry loop (U4)
/// that retries only what is genuinely incomplete - an `AppUnresponsive`
/// error stamped explicitly retryable (an unreadable candidate, a vanished
/// or transient node mid-descent). A settled answer - `STALE_REF` from a
/// completed search, `AMBIGUOUS_TARGET`, a permission denial - is never
/// retried. Every re-attempt re-verifies process liveness through
/// `resolve_window_root` (A14-4's prescribed cure), so a dead process
/// converts to settled `STALE_REF` on the next attempt instead of burning
/// the deadline, and final expiry stamps `deadline_elapsed` onto the last
/// incomplete diagnosis rather than a bare `TIMEOUT` that discards it.
#[cfg(target_os = "windows")]
pub(crate) fn resolve_element_strict(
    entry: &RefEntry,
    deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    retry_incomplete_until(deadline, || resolve_attempt(entry, deadline))
}

#[cfg(target_os = "windows")]
fn resolve_attempt(entry: &RefEntry, deadline: Deadline) -> Result<NativeHandle, AdapterError> {
    let root = resolve_window_root(entry, deadline)?;
    let source = UiaTreeSource::for_root(&root)?;
    let prepared = source.prepare_root(&root)?;

    let budget = WalkBudget::new(MAX_RESOLVE_DEPTH, deadline)
        .with_max_raw_depth(MAX_RESOLVE_DEPTH)
        .with_max_siblings(DEFAULT_MAX_SIBLINGS);

    // The path fast-path (see `resolve_search`): a locator, never identity.
    if can_use_path_fast_path(entry) {
        let mut path_incomplete = false;
        if let Some(candidate) = element_at_path(
            &source,
            &prepared,
            &entry.scope.path,
            &budget,
            &mut path_incomplete,
        )? {
            let (_, evidence, _) = source.evidence(&candidate);
            let role_matches = evidence
                .role
                .known()
                .is_some_and(|role| role == &entry.identity.role);
            if role_matches {
                match super::resolve_match::candidate_outcome(entry, &evidence) {
                    CandidateOutcome::Matched => return Ok(candidate.into_native_handle()),
                    CandidateOutcome::Incomplete if geometry_matches(entry, &evidence) => {
                        return Ok(candidate.into_native_handle());
                    }
                    _ => {}
                }
            }
        }
    }

    let mut searched = Vec::new();
    let mut incomplete = false;
    search_under(
        &source,
        &prepared,
        0,
        &budget,
        entry,
        &mut searched,
        &mut incomplete,
    )?;

    match searched.len() {
        0 if incomplete => Err(identity_unknown_error(entry)),
        0 => Err(stale_ref_error(entry)),
        1 => {
            let Some(candidate) = searched.into_iter().next() else {
                return Err(stale_ref_error(entry));
            };
            Ok(candidate.element.into_native_handle())
        }
        _ => {
            let candidate_hashes: Vec<Option<u64>> = searched
                .iter()
                .map(|candidate| candidate.bounds_hash)
                .collect();
            match super::resolve_match::select_by_bounds_hash(
                &candidate_hashes,
                entry.geometry.bounds_hash,
            ) {
                super::resolve_match::Selection::Resolved(index) => {
                    Ok(searched[index].element.clone().into_native_handle())
                }
                super::resolve_match::Selection::Ambiguous => {
                    Err(ambiguous_target_error(entry, searched.len()))
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn retry_incomplete_until(
    deadline: Deadline,
    mut operation: impl FnMut() -> Result<NativeHandle, AdapterError>,
) -> Result<NativeHandle, AdapterError> {
    let mut last_incomplete: Option<AdapterError> = None;
    loop {
        if deadline.is_expired() {
            return Err(last_incomplete
                .map(mark_deadline_elapsed)
                .unwrap_or_else(|| deadline.timeout_error()));
        }
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if is_retryable_resolution_error(&error) => {
                last_incomplete = Some(error);
                sleep_before_retry(deadline);
            }
            Err(error) if error.code == ErrorCode::Timeout => {
                return Err(match last_incomplete {
                    Some(incomplete) => mark_deadline_elapsed(incomplete),
                    None => error,
                });
            }
            Err(error) => return Err(error),
        }
    }
}

/// Whether the adapter's own loop should retry an error: only an explicitly
/// retryable, incomplete read (U4). Mirrors macOS
/// (`resolve.rs:275-277`); the granularity split between adapter-loop-settled
/// and incomplete lives in the `complete`/`retryable` details every resolver
/// error carries.
#[cfg(target_os = "windows")]
fn is_retryable_resolution_error(error: &AdapterError) -> bool {
    error.code == ErrorCode::AppUnresponsive && error.is_explicitly_retryable()
}

#[cfg(target_os = "windows")]
fn sleep_before_retry(deadline: Deadline) {
    let remaining = deadline.remaining();
    std::thread::sleep(remaining.min(std::time::Duration::from_millis(25)));
}

/// Stamps the final incomplete diagnosis with `deadline_elapsed` so the
/// caller sees why the retries ran out, preserving the incomplete's own
/// details rather than discarding them for a bare `TIMEOUT`.
#[cfg(target_os = "windows")]
fn mark_deadline_elapsed(mut error: AdapterError) -> AdapterError {
    let mut details = error.details.take().unwrap_or_else(|| serde_json::json!({}));
    if let Some(object) = details.as_object_mut() {
        object.insert("deadline_elapsed".into(), serde_json::json!(true));
    } else {
        details = serde_json::json!({
            "evidence": details,
            "deadline_elapsed": true,
        });
    }
    error.with_details(details)
}
/// The non-Windows twin. The crate cross-compiles to the Linux lane with the
/// resolver reachable, but there are no UI Automation elements there, so every
/// stored ref fails closed as stale rather than attempting a search that
/// cannot find anything.
#[cfg(not(target_os = "windows"))]
pub(crate) fn resolve_element_strict(
    entry: &RefEntry,
    _deadline: Deadline,
) -> Result<NativeHandle, AdapterError> {
    Err(stale_ref_error(entry))
}

/// Reaches the stored window's root element from the ref's source window id.
///
/// The fail-closed process gate (the A7-3 wrong-target shape exists to
/// prevent): a stored ref must not search the tree of a different process that
/// has since recycled the HWND. The macOS resolver verifies process instance
/// before searching either. A token-less ref (elevated process whose token
/// could not be read) fails closed here rather than searching an unverified
/// window.
#[cfg(target_os = "windows")]
fn resolve_window_root(entry: &RefEntry, deadline: Deadline) -> Result<UIAElement, AdapterError> {
    let window_id = entry
        .source
        .source_window_id
        .as_deref()
        .ok_or_else(|| stale_ref_error(entry))?;
    if let Some(instance) = entry.process.process_instance.as_deref() {
        if !crate::system::process_identity::matches_instance(entry.process.pid, instance)? {
            return Err(stale_ref_error(entry));
        }
    } else {
        return Err(stale_ref_error(entry));
    }
    crate::tree::surfaces::surface_root(
        agent_desktop_core::ObservationRoot::Window(&agent_desktop_core::WindowInfo {
            id: window_id.to_string(),
            title: entry.source.source_window_title.clone().unwrap_or_default(),
            app: entry.source.source_app.clone().unwrap_or_default(),
            pid: entry.process.pid,
            process_instance: entry.process.process_instance.clone(),
            bounds: None,
            state: Default::default(),
        }),
        entry.source.source_surface,
        deadline,
    )
}

#[cfg(all(test, target_os = "windows"))]
#[path = "resolve_tests.rs"]
mod windows_only;
