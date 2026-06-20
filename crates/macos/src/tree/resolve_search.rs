use agent_desktop_core::{adapter::NativeHandle, error::AdapterError, refs::RefEntry};
use rustc_hash::FxHashSet;
use std::time::Instant;

use super::AXElement;
#[cfg(target_os = "macos")]
use super::attributes::{copy_ax_array, copy_string_attr, set_messaging_timeout};
#[cfg(target_os = "macos")]
use super::element::{child_attributes, resolve_element_name};
use super::element_dedupe::ElementDedupe;
use super::resolve_bounds::should_prune_by_bounds;
#[cfg(target_os = "macos")]
use super::resolve_classify::classify_candidates;
#[cfg(target_os = "macos")]
use super::resolve_deadline::{ensure_before_deadline, remaining_before_deadline};
use super::resolve_identity::has_meaningful_identity;
#[cfg(target_os = "macos")]
use super::resolve_identity::identity_matches;

#[cfg(target_os = "macos")]
pub(super) fn find_entry_by_path(
    roots: &[AXElement],
    entry: &RefEntry,
    source_window_verified: bool,
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

    classify_candidates(matches, entry, source_window_verified)
}

#[cfg(target_os = "macos")]
fn element_at_path(
    root: &AXElement,
    path: &[usize],
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let mut current = root.clone();
    let mut seen = FxHashSet::default();
    for idx in path {
        ensure_before_deadline(deadline)?;
        let ax_role = copy_string_attr(&current, accessibility_sys::kAXRoleAttribute);
        let children = resolve_children(&current, ax_role.as_deref(), deadline, &mut seen)?;
        let Some(child) = children.get(*idx) else {
            return Ok(None);
        };
        current = child.clone();
    }
    Ok(Some(current))
}

#[cfg(target_os = "macos")]
pub(super) fn find_entry_in_roots(
    roots: &[AXElement],
    entry: &RefEntry,
    resolve_depth: u8,
    source_window_verified: bool,
    deadline: Instant,
) -> Result<NativeHandle, AdapterError> {
    let mut matches = Vec::new();
    let mut seen_matches = ElementDedupe;
    let mut child_scratch = FxHashSet::default();
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
            child_scratch: &mut child_scratch,
            deadline,
        };
        collect_elements_recursive(root, 0, &mut context)?;
    }
    classify_candidates(matches, entry, source_window_verified)
}

#[cfg(target_os = "macos")]
struct CollectContext<'a> {
    entry: &'a RefEntry,
    max_depth: u8,
    ancestors: &'a mut FxHashSet<usize>,
    seen_matches: &'a mut ElementDedupe,
    matches: &'a mut Vec<AXElement>,
    child_scratch: &'a mut FxHashSet<usize>,
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
    let (normalized, promoted_label) =
        crate::tree::roles::normalized_role_and_label(el, ax_role.as_deref());

    if normalized == context.entry.role
        && element_identity_matches(el, context.entry, promoted_label)
        && context.seen_matches.push_clone(context.matches, el)
        && should_stop_collecting(context.matches.len(), context.entry)
    {
        context.ancestors.remove(&ptr_key);
        return Ok(());
    }

    if depth < context.max_depth && !should_prune_for_resolution(el, context.entry, depth) {
        let children = resolve_children(
            el,
            ax_role.as_deref(),
            context.deadline,
            context.child_scratch,
        )?;
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
    let (normalized, promoted_label) =
        crate::tree::roles::normalized_role_and_label(el, ax_role.as_deref());
    normalized == entry.role && element_identity_matches(el, entry, promoted_label)
}

pub(super) fn should_stop_collecting(match_count: usize, entry: &RefEntry) -> bool {
    match_count > 1 && entry.bounds_hash.is_none()
}

fn should_prune_for_resolution(el: &AXElement, entry: &RefEntry, depth: u8) -> bool {
    !has_meaningful_identity(entry) && should_prune_by_bounds(el, entry, depth)
}

#[cfg(target_os = "macos")]
fn element_identity_matches(
    el: &AXElement,
    entry: &RefEntry,
    promoted_label: Option<String>,
) -> bool {
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
    seen: &mut FxHashSet<usize>,
) -> Result<Vec<AXElement>, AdapterError> {
    seen.clear();
    set_messaging_timeout(el, remaining_before_deadline(deadline)?);
    let mut result = Vec::new();
    for attr in child_attributes(ax_role) {
        ensure_before_deadline(deadline)?;
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
