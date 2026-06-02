use agent_desktop_core::{
    adapter::{NativeHandle, SnapshotSurface},
    error::{AdapterError, ErrorCode},
    refs::RefEntry,
};
use rustc_hash::FxHashSet;

use super::AXElement;
use super::builder::window_element_for;
use super::element::{
    child_attributes, copy_ax_array, copy_element_attr, copy_i64_attr, copy_string_attr,
    element_for_pid, resolve_element_name,
};
use super::resolve_bounds::{bounds_match, should_prune_by_bounds};
use super::resolve_identity::{has_meaningful_identity, identity_matches};

#[cfg(target_os = "macos")]
pub fn resolve_element_impl(entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    tracing::debug!(
        "resolve: searching pid={} role={} name={:?} description={:?} bounds_hash={:?}",
        entry.pid,
        entry.role,
        entry.name.as_deref().unwrap_or("(none)"),
        entry.description.as_deref().unwrap_or("(none)"),
        entry.bounds_hash
    );
    let resolve_depth: u8 = 50;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let attempts = 4;
    for attempt in 0..attempts {
        if can_use_path_fast_path(entry) {
            let path_roots = path_candidate_roots(entry);
            if let Ok(handle) = find_entry_by_path(&path_roots, entry) {
                tracing::debug!("resolve: found path match");
                return Ok(handle);
            }
            if should_retry_scoped_path_resolution(entry) {
                if attempt + 1 < attempts && std::time::Instant::now() < deadline {
                    std::thread::sleep(std::time::Duration::from_millis(75));
                }
                continue;
            }
        }
        if !can_use_broad_search(entry) {
            if attempt + 1 < attempts && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(75));
            }
            continue;
        }
        let roots = candidate_roots(entry);
        if let Ok(handle) = find_entry_in_roots(&roots, entry, resolve_depth, deadline) {
            tracing::debug!("resolve: found exact match");
            return Ok(handle);
        }

        if attempt + 1 < attempts && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(75));
        }
    }

    tracing::debug!("resolve: element not found");
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
fn can_use_path_fast_path(entry: &RefEntry) -> bool {
    (entry.root_ref.is_none() || entry.path_is_absolute)
        && !entry.path.is_empty()
        && (entry.bounds_hash.is_some()
            || entry.source_window_id.is_some()
            || entry.source_window_title.is_some())
}

#[cfg(target_os = "macos")]
fn requires_scoped_path_resolution(entry: &RefEntry) -> bool {
    (entry.root_ref.is_none() || entry.path_is_absolute)
        && entry.bounds_hash.is_none()
        && !entry.path.is_empty()
        && (entry.source_window_id.is_some() || entry.source_window_title.is_some())
}

#[cfg(target_os = "macos")]
fn should_retry_scoped_path_resolution(entry: &RefEntry) -> bool {
    requires_scoped_path_resolution(entry)
}

#[cfg(target_os = "macos")]
fn can_use_broad_search(entry: &RefEntry) -> bool {
    entry.bounds_hash.is_some() || has_meaningful_identity(entry)
}

#[cfg(target_os = "macos")]
fn path_candidate_roots(entry: &RefEntry) -> Vec<AXElement> {
    if entry.bounds_hash.is_some() {
        return candidate_roots(entry);
    }
    scoped_surface_root(entry).into_iter().collect()
}

#[cfg(target_os = "macos")]
fn scoped_surface_root(entry: &RefEntry) -> Option<AXElement> {
    match entry.source_surface {
        SnapshotSurface::Window => exact_source_window_root(entry),
        SnapshotSurface::Focused => crate::tree::focused_surface_for_pid(entry.pid),
        SnapshotSurface::Menu => crate::tree::menu_element_for_pid(entry.pid),
        SnapshotSurface::Menubar => crate::tree::menubar_for_pid(entry.pid),
        SnapshotSurface::Sheet => crate::tree::sheet_for_pid(entry.pid),
        SnapshotSurface::Popover => crate::tree::popover_for_pid(entry.pid),
        SnapshotSurface::Alert => crate::tree::alert_for_pid(entry.pid),
    }
}

#[cfg(target_os = "macos")]
fn exact_source_window_root(entry: &RefEntry) -> Option<AXElement> {
    let root = element_for_pid(entry.pid);
    let windows = copy_ax_array(&root, "AXWindows")?;
    if let Some(source_window_number) = source_window_number(entry) {
        if let Some(window) = windows
            .iter()
            .find(|win| copy_i64_attr(win, "AXWindowNumber") == Some(source_window_number))
        {
            return Some(window.clone());
        }
    }
    let source_window_title = entry.source_window_title.as_deref()?;
    windows
        .into_iter()
        .find(|win| copy_string_attr(win, "AXTitle").as_deref() == Some(source_window_title))
}

#[cfg(target_os = "macos")]
fn source_window_number(entry: &RefEntry) -> Option<i64> {
    entry
        .source_window_id
        .as_deref()?
        .strip_prefix("w-")?
        .parse()
        .ok()
}

#[cfg(target_os = "macos")]
fn candidate_roots(entry: &RefEntry) -> Vec<AXElement> {
    let root = element_for_pid(entry.pid);
    let mut roots = Vec::new();
    if let Some(source_window_title) = entry.source_window_title.as_deref() {
        roots.push(window_element_for(entry.pid, source_window_title));
    }
    if let Some(focused) = copy_element_attr(&root, "AXFocusedWindow") {
        roots.push(focused);
    }
    if let Some(main) = copy_element_attr(&root, "AXMainWindow") {
        roots.push(main);
    }
    roots.extend(copy_ax_array(&root, "AXWindows").unwrap_or_default());
    if let Some(menubar) = crate::tree::menubar_for_pid(entry.pid) {
        roots.push(menubar);
    }
    if let Some(menu) = crate::tree::menu_element_for_pid(entry.pid) {
        roots.push(menu);
    }
    if roots.is_empty() {
        roots.push(root);
    }
    roots
}

#[cfg(target_os = "macos")]
fn find_entry_by_path(roots: &[AXElement], entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    if entry.path.is_empty() {
        return Err(AdapterError::element_not_found("element"));
    }

    let mut matches = Vec::new();
    let mut seen = FxHashSet::default();
    for root in roots {
        let Some(candidate) = element_at_path(root, &entry.path) else {
            continue;
        };
        if element_matches_entry(&candidate, entry) && seen.insert(candidate.0 as usize) {
            matches.push(candidate);
        }
    }

    classify_candidates(matches, entry)
}

#[cfg(target_os = "macos")]
fn element_at_path(root: &AXElement, path: &[usize]) -> Option<AXElement> {
    let mut current = root.clone();
    for idx in path {
        let ax_role = copy_string_attr(&current, accessibility_sys::kAXRoleAttribute);
        let children = resolve_children(&current, ax_role.as_deref());
        current = children.get(*idx)?.clone();
    }
    Some(current)
}

#[cfg(target_os = "macos")]
fn find_entry_in_roots(
    roots: &[AXElement],
    entry: &RefEntry,
    resolve_depth: u8,
    deadline: std::time::Instant,
) -> Result<NativeHandle, AdapterError> {
    let mut matches = Vec::new();
    let mut seen_matches = FxHashSet::default();
    for root in roots {
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
    use core_foundation::base::{CFRetain, CFTypeRef};

    match matches.len() {
        0 => Err(AdapterError::element_not_found("element")),
        1 => {
            let candidate = matches.remove(0);
            unsafe { CFRetain(candidate.0 as CFTypeRef) };
            Ok(unsafe { NativeHandle::from_ptr(candidate.0 as *const _) })
        }
        count => Err(AdapterError::ambiguous_target(format!(
            "Ambiguous target: {count} candidates matched role={}, name={:?}, description={:?}",
            entry.role,
            entry.name.as_deref().unwrap_or("(none)"),
            entry.description.as_deref().unwrap_or("(none)")
        ))),
    }
}

#[cfg(target_os = "macos")]
struct CollectContext<'a> {
    entry: &'a RefEntry,
    max_depth: u8,
    ancestors: &'a mut FxHashSet<usize>,
    seen_matches: &'a mut FxHashSet<usize>,
    matches: &'a mut Vec<AXElement>,
    deadline: std::time::Instant,
}

#[cfg(target_os = "macos")]
fn collect_elements_recursive(
    el: &AXElement,
    depth: u8,
    context: &mut CollectContext<'_>,
) -> Result<(), AdapterError> {
    use accessibility_sys::kAXRoleAttribute;

    if std::time::Instant::now() > context.deadline {
        return Err(
            AdapterError::new(ErrorCode::Timeout, "Element resolution timed out")
                .with_suggestion("Retry the command, or run 'snapshot' if the UI changed."),
        );
    }

    let ptr_key = el.0 as usize;
    if !context.ancestors.insert(ptr_key) {
        return Ok(());
    }

    let ax_role = copy_string_attr(el, kAXRoleAttribute);
    let normalized = crate::tree::roles::normalized_role_for_element(el, ax_role.as_deref());

    if normalized == context.entry.role
        && element_matches_entry(el, context.entry)
        && context.seen_matches.insert(ptr_key)
    {
        context.matches.push(el.clone());
    }

    if depth < context.max_depth && !should_prune_by_bounds(el, context.entry, depth) {
        let children = resolve_children(el, ax_role.as_deref());
        for child in &children {
            collect_elements_recursive(child, depth + 1, context)?;
        }
    }

    context.ancestors.remove(&ptr_key);
    Ok(())
}

#[cfg(target_os = "macos")]
fn element_matches_entry(el: &AXElement, entry: &RefEntry) -> bool {
    element_matches_path_entry(el, entry) && bounds_match(el, entry)
}

#[cfg(target_os = "macos")]
fn element_matches_path_entry(el: &AXElement, entry: &RefEntry) -> bool {
    let ax_role = copy_string_attr(el, accessibility_sys::kAXRoleAttribute);
    let (normalized, promoted_label) =
        crate::tree::roles::normalized_role_and_label(el, ax_role.as_deref());
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
fn resolve_children(el: &AXElement, ax_role: Option<&str>) -> Vec<AXElement> {
    let mut seen = FxHashSet::default();
    let mut result = Vec::new();
    for attr in child_attributes(ax_role) {
        if let Some(children) = copy_ax_array(el, attr) {
            for child in children {
                if seen.insert(child.0 as usize) {
                    result.push(child);
                }
            }
        }
    }
    result
}

#[cfg(test)]
#[path = "resolve_tests.rs"]
mod tests;

#[cfg(not(target_os = "macos"))]
pub fn resolve_element_impl(_entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    Err(AdapterError::not_supported("resolve_element"))
}
