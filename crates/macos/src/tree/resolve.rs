use agent_desktop_core::{
    adapter::{NativeHandle, SnapshotSurface},
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
};
use rustc_hash::FxHashSet;
use std::time::{Duration, Instant};

use super::AXElement;
use super::attributes::{copy_ax_array, copy_string_attr, set_messaging_timeout};
use super::element::{child_attributes, resolve_element_name};
use super::element_dedupe::ElementDedupe;
use super::resolve_bounds::{bounds_match, should_prune_by_bounds};
use super::resolve_deadline::{
    ensure_before_deadline, remaining_before_deadline, sleep_before_retry,
};
use super::resolve_identity::{has_meaningful_identity, identity_matches};
use super::resolve_roots::{candidate_roots, path_candidate_roots, source_window_number};

#[cfg(target_os = "macos")]
pub fn resolve_element_impl(entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    resolve_element_with_timeout(entry, Duration::from_secs(5))
}

#[cfg(target_os = "macos")]
pub fn resolve_element_with_timeout(
    entry: &RefEntry,
    timeout: Duration,
) -> Result<NativeHandle, AdapterError> {
    let (resolve_depth, attempts) = (50, 4);
    let deadline = Instant::now() + timeout;
    for attempt in 0..attempts {
        if can_use_path_fast_path(entry) {
            let path_roots = path_candidate_roots(entry, deadline)?;
            match find_entry_by_path(&path_roots, entry, deadline) {
                Ok(handle) => {
                    return Ok(handle);
                }
                Err(err) if is_retryable_resolution_error(&err) => {}
                Err(err) => return Err(err),
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
        let roots = candidate_roots(entry, deadline)?;
        match find_entry_in_roots(&roots, entry, resolve_depth, deadline) {
            Ok(handle) => {
                return Ok(handle);
            }
            Err(err) if is_retryable_resolution_error(&err) => {}
            Err(err) => return Err(err),
        }

        if attempt + 1 < attempts {
            sleep_before_retry(deadline);
        }
    }

    Err(AdapterError::new(
        ErrorCode::StaleRef,
        format!(
            "Element not found: role={}, name={:?}, description={:?}",
            entry.role,
            entry.name.as_deref().unwrap_or("(none)"),
            entry.description.as_deref().unwrap_or("(none)")
        ),
    )
    .with_suggestion("Run 'snapshot' to refresh, then retry with the updated ref."))
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
        && source_window_number(entry).is_some()
}

#[cfg(target_os = "macos")]
fn can_use_broad_search(entry: &RefEntry) -> bool {
    entry.bounds_hash.is_some() || has_meaningful_identity(entry)
}

#[cfg(target_os = "macos")]
fn find_entry_by_path(
    roots: &[AXElement],
    entry: &RefEntry,
    deadline: Instant,
) -> Result<NativeHandle, AdapterError> {
    if entry.path.is_empty() {
        return Err(AdapterError::element_not_found("element"));
    }
    ensure_before_deadline(deadline)?;

    let mut matches = Vec::new();
    let mut dedupe = ElementDedupe;
    for root in roots {
        ensure_before_deadline(deadline)?;
        if should_stop_collecting(matches.len(), entry) {
            break;
        }
        let Some(candidate) = element_at_path(root, &entry.path, deadline)? else {
            continue;
        };
        if element_matches_entry(&candidate, entry) {
            dedupe.push(&mut matches, candidate);
        }
    }

    classify_candidates(matches, entry)
}

#[cfg(target_os = "macos")]
fn element_at_path(
    root: &AXElement,
    path: &[usize],
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let mut current = root.clone();
    for idx in path {
        ensure_before_deadline(deadline)?;
        set_messaging_timeout(&current, remaining_before_deadline(deadline)?);
        let ax_role = copy_string_attr(&current, accessibility_sys::kAXRoleAttribute);
        let children = resolve_children(&current, ax_role.as_deref(), deadline)?;
        let Some(child) = children.get(*idx) else {
            return Ok(None);
        };
        current = child.clone();
    }
    Ok(Some(current))
}

#[cfg(target_os = "macos")]
fn find_entry_in_roots(
    roots: &[AXElement],
    entry: &RefEntry,
    resolve_depth: u8,
    deadline: Instant,
) -> Result<NativeHandle, AdapterError> {
    let mut matches = Vec::new();
    let mut seen_matches = ElementDedupe;
    for root in roots {
        if should_stop_collecting(matches.len(), entry) {
            break;
        }
        let mut visited = FxHashSet::default();
        let mut context = CollectContext {
            entry,
            max_depth: resolve_depth,
            ancestors: &mut visited,
            seen_matches: &mut seen_matches,
            matches: &mut matches,
            deadline,
        };
        collect_elements_recursive(root, 0, &mut context)?;
    }
    classify_candidates(matches, entry)
}

#[cfg(target_os = "macos")]
fn classify_candidates(
    mut matches: Vec<AXElement>,
    entry: &RefEntry,
) -> Result<NativeHandle, AdapterError> {
    match matches.len() {
        0 => Err(AdapterError::element_not_found("element")),
        1 => {
            let candidate = matches.remove(0);
            if source_window_scope_verifies_lone_match(entry)
                || verified_bounds_match(&candidate, entry)
            {
                retained_handle(candidate)
            } else {
                Err(AdapterError::element_not_found("element"))
            }
        }
        _ => classify_ambiguous_candidates(matches, entry),
    }
}

#[cfg(target_os = "macos")]
fn source_window_scope_verifies_lone_match(entry: &RefEntry) -> bool {
    matches!(entry.source_surface, SnapshotSurface::Window) && source_window_number(entry).is_some()
}

#[cfg(target_os = "macos")]
fn verified_bounds_match(candidate: &AXElement, entry: &RefEntry) -> bool {
    entry.bounds_hash.is_some() && bounds_match(candidate, entry)
}

#[cfg(target_os = "macos")]
fn classify_ambiguous_candidates(
    matches: Vec<AXElement>,
    entry: &RefEntry,
) -> Result<NativeHandle, AdapterError> {
    let mut bounds_matches: Vec<_> = matches
        .iter()
        .filter(|candidate| verified_bounds_match(candidate, entry))
        .cloned()
        .collect();
    if bounds_matches.len() == 1 {
        return retained_handle(bounds_matches.remove(0));
    }
    let count = matches.len();
    Err(AdapterError::ambiguous_target(format!(
        "Ambiguous target: {count} candidates matched role={}, name={:?}, description={:?}",
        entry.role,
        entry.name.as_deref().unwrap_or("(none)"),
        entry.description.as_deref().unwrap_or("(none)")
    ))
    .with_details(serde_json::json!({
        "candidate_count": count,
        "role": entry.role,
        "name": entry.name,
        "description": entry.description,
        "source_app": entry.source_app,
        "source_window_id": entry.source_window_id,
        "source_window_title": entry.source_window_title,
        "candidates": candidate_summaries(&matches)
    })))
}

#[cfg(target_os = "macos")]
fn retained_handle(candidate: AXElement) -> Result<NativeHandle, AdapterError> {
    use core_foundation::base::{CFRetain, CFTypeRef};
    #[cfg(test)]
    if candidate.0.is_null() {
        return Ok(NativeHandle::null());
    }
    unsafe { CFRetain(candidate.0 as CFTypeRef) };
    Ok(unsafe { NativeHandle::from_ptr(candidate.0 as *const _) })
}

#[cfg(target_os = "macos")]
fn candidate_summaries(matches: &[AXElement]) -> Vec<serde_json::Value> {
    matches
        .iter()
        .take(10)
        .enumerate()
        .map(|(index, element)| {
            let ax_role = copy_string_attr(element, accessibility_sys::kAXRoleAttribute);
            let role = crate::tree::roles::normalized_role_for_element(element, ax_role.as_deref());
            let name = crate::tree::roles::normalized_role_and_label(element, ax_role.as_deref())
                .1
                .or_else(|| resolve_element_name(element));
            let description = copy_string_attr(element, accessibility_sys::kAXDescriptionAttribute);
            let bounds = crate::tree::read_bounds(element);
            serde_json::json!({
                "index": index,
                "role": role,
                "name": name,
                "description": description,
                "bounds": bounds,
                "bounds_hash": bounds.as_ref().map(|bounds| bounds.bounds_hash())
            })
        })
        .collect()
}

#[cfg(target_os = "macos")]
struct CollectContext<'a> {
    entry: &'a RefEntry,
    max_depth: u8,
    ancestors: &'a mut FxHashSet<usize>,
    seen_matches: &'a mut ElementDedupe,
    matches: &'a mut Vec<AXElement>,
    deadline: Instant,
}

#[cfg(target_os = "macos")]
fn collect_elements_recursive(
    el: &AXElement,
    depth: u8,
    context: &mut CollectContext<'_>,
) -> Result<(), AdapterError> {
    use accessibility_sys::kAXRoleAttribute;

    if should_stop_collecting(context.matches.len(), context.entry) {
        return Ok(());
    }
    ensure_before_deadline(context.deadline)?;

    let ptr_key = el.0 as usize;
    if !context.ancestors.insert(ptr_key) {
        return Ok(());
    }

    let ax_role = copy_string_attr(el, kAXRoleAttribute);
    let normalized = crate::tree::roles::normalized_role_for_element(el, ax_role.as_deref());

    if normalized == context.entry.role
        && element_matches_path_entry_with_role(el, context.entry, ax_role.as_deref())
        && context.seen_matches.push_clone(context.matches, el)
    {
        if should_stop_collecting(context.matches.len(), context.entry) {
            context.ancestors.remove(&ptr_key);
            return Ok(());
        }
    }

    if depth < context.max_depth && !should_prune_for_resolution(el, context.entry, depth) {
        let children = resolve_children(el, ax_role.as_deref(), context.deadline)?;
        for child in &children {
            collect_elements_recursive(child, depth + 1, context)?;
        }
    }

    context.ancestors.remove(&ptr_key);
    Ok(())
}

#[cfg(target_os = "macos")]
fn element_matches_entry(el: &AXElement, entry: &RefEntry) -> bool {
    let ax_role = copy_string_attr(el, accessibility_sys::kAXRoleAttribute);
    element_matches_path_entry_with_role(el, entry, ax_role.as_deref())
}

fn should_stop_collecting(match_count: usize, entry: &RefEntry) -> bool {
    match_count > 1 && entry.bounds_hash.is_none()
}

fn should_prune_for_resolution(el: &AXElement, entry: &RefEntry, depth: u8) -> bool {
    !has_meaningful_identity(entry) && should_prune_by_bounds(el, entry, depth)
}

#[cfg(target_os = "macos")]
fn element_matches_path_entry_with_role(
    el: &AXElement,
    entry: &RefEntry,
    ax_role: Option<&str>,
) -> bool {
    let (normalized, promoted_label) = crate::tree::roles::normalized_role_and_label(el, ax_role);
    if normalized != entry.role {
        return false;
    }

    let elem_name = promoted_label.or_else(|| resolve_element_name(el));
    let elem_value = crate::tree::copy_value_typed(el);
    let elem_description = copy_string_attr(el, accessibility_sys::kAXDescriptionAttribute);
    identity_matches(
        entry,
        elem_name.as_deref(),
        elem_value.as_deref(),
        elem_description.as_deref(),
    )
}

#[cfg(target_os = "macos")]
fn resolve_children(
    el: &AXElement,
    ax_role: Option<&str>,
    deadline: Instant,
) -> Result<Vec<AXElement>, AdapterError> {
    let mut seen = FxHashSet::default();
    let mut result = Vec::new();
    for attr in child_attributes(ax_role) {
        ensure_before_deadline(deadline)?;
        set_messaging_timeout(el, remaining_before_deadline(deadline)?);
        if let Some(children) = copy_ax_array(el, attr) {
            for child in children {
                if seen.insert(child.0 as usize) {
                    result.push(child);
                }
            }
        }
    }
    Ok(result)
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;

#[cfg(not(target_os = "macos"))]
pub fn resolve_element_impl(_entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    Err(AdapterError::not_supported("resolve_element"))
}
