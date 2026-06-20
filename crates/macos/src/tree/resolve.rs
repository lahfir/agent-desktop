use agent_desktop_core::{
    adapter::NativeHandle,
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use super::resolve_classify::identity_summary_for_message;
#[cfg(target_os = "macos")]
use super::resolve_deadline::sleep_before_retry;
#[cfg(target_os = "macos")]
use super::resolve_identity::has_meaningful_identity;
#[cfg(target_os = "macos")]
use super::resolve_roots::{
    candidate_roots, path_candidate_roots, source_window_number, source_window_scope_required,
};
#[cfg(target_os = "macos")]
use super::resolve_search::{find_entry_by_path, find_entry_in_roots};

#[cfg(target_os = "macos")]
pub fn resolve_element_impl(entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    resolve_element_with_timeout(entry, Duration::from_secs(5))
}

/// Resolves a ref to a live handle, retrying until the deadline. Once any
/// complete search pass concludes the element is absent, a later attempt that
/// merely runs out of deadline budget is still a stale ref — not an
/// indeterminate timeout. Tracking `proved_absent` keeps STALE_REF
/// deterministic regardless of how big or slow the surrounding tree is.
#[cfg(target_os = "macos")]
pub fn resolve_element_with_timeout(
    entry: &RefEntry,
    timeout: Duration,
) -> Result<NativeHandle, AdapterError> {
    let (resolve_depth, attempts) = (50, 4);
    let deadline = Instant::now() + timeout;
    let mut proved_absent = false;
    for attempt in 0..attempts {
        if can_use_path_fast_path(entry) {
            let path_roots = match path_candidate_roots(entry, deadline) {
                Ok(roots) => roots,
                Err(err) => return Err(downgrade_timeout(err, entry, proved_absent)),
            };
            let scope_verified = path_roots.scope_verified;
            match find_entry_by_path(&path_roots.roots, entry, scope_verified, deadline) {
                Ok(handle) => {
                    return Ok(handle);
                }
                Err(err) if is_retryable_resolution_error(&err) => proved_absent = true,
                Err(err) => return Err(downgrade_timeout(err, entry, proved_absent)),
            }
            if requires_scoped_path_resolution(entry) {
                if attempt + 1 < attempts {
                    sleep_before_retry(deadline);
                }
                continue;
            }
        }
        if !can_use_broad_search(entry) {
            if attempt + 1 < attempts {
                sleep_before_retry(deadline);
            }
            continue;
        }
        let roots = match candidate_roots(entry, deadline) {
            Ok(roots) => roots,
            Err(err) => return Err(downgrade_timeout(err, entry, proved_absent)),
        };
        let scope_verified = roots.scope_verified;
        match find_entry_in_roots(&roots.roots, entry, resolve_depth, scope_verified, deadline) {
            Ok(handle) => {
                return Ok(handle);
            }
            Err(err) if is_retryable_resolution_error(&err) => proved_absent = true,
            Err(err) => return Err(downgrade_timeout(err, entry, proved_absent)),
        }

        if attempt + 1 < attempts {
            sleep_before_retry(deadline);
        }
    }

    Err(stale_ref_error(entry))
}

#[cfg(target_os = "macos")]
fn stale_ref_error(entry: &RefEntry) -> AdapterError {
    AdapterError::new(
        ErrorCode::StaleRef,
        format!("Element not found: {}", identity_summary_for_message(entry)),
    )
    .with_suggestion("Run 'snapshot' to refresh, then retry with the updated ref.")
}

/// A deadline `TIMEOUT` becomes `STALE_REF` once a prior complete pass proved
/// the element absent: the element is gone, the timeout is incidental.
#[cfg(target_os = "macos")]
fn downgrade_timeout(err: AdapterError, entry: &RefEntry, proved_absent: bool) -> AdapterError {
    if proved_absent && err.code == ErrorCode::Timeout {
        stale_ref_error(entry)
    } else {
        err
    }
}

#[cfg(target_os = "macos")]
fn is_retryable_resolution_error(err: &AdapterError) -> bool {
    err.code == ErrorCode::ElementNotFound
}

#[cfg(target_os = "macos")]
fn can_use_path_fast_path(entry: &RefEntry) -> bool {
    (entry.root_ref.is_none() || entry.path_is_absolute)
        && !entry.path.is_empty()
        && (entry.bounds_hash.is_some() || source_window_number(entry).is_some())
}

#[cfg(target_os = "macos")]
fn requires_scoped_path_resolution(entry: &RefEntry) -> bool {
    (entry.root_ref.is_none() || entry.path_is_absolute)
        && entry.bounds_hash.is_none()
        && !entry.path.is_empty()
        && source_window_scope_required(entry)
}

#[cfg(target_os = "macos")]
fn can_use_broad_search(entry: &RefEntry) -> bool {
    entry.bounds_hash.is_some() || has_meaningful_identity(entry)
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;

#[cfg(not(target_os = "macos"))]
pub fn resolve_element_impl(_entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    Err(AdapterError::not_supported("resolve_element"))
}
