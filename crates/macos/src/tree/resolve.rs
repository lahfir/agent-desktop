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
            if requires_scoped_path_resolution(entry) {
                if attempt + 1 < attempts && std::time::Instant::now() < deadline {
                    std::thread::sleep(std::time::Duration::from_millis(75));
                }
                continue;
            }
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
    use core_foundation::base::{CFRetain, CFTypeRef};

    if entry.path.is_empty() {
        return Err(AdapterError::element_not_found("element"));
    }

    for root in roots {
        let Some(candidate) = element_at_path(root, &entry.path) else {
            continue;
        };
        if element_matches_entry(&candidate, entry) {
            unsafe { CFRetain(candidate.0 as CFTypeRef) };
            return Ok(unsafe { NativeHandle::from_ptr(candidate.0 as *const _) });
        }
    }

    Err(AdapterError::element_not_found("element"))
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
    for root in roots {
        let mut visited = FxHashSet::default();
        if let Ok(handle) =
            find_element_recursive(root, entry, 0, resolve_depth, &mut visited, deadline)
        {
            return Ok(handle);
        }
    }
    Err(AdapterError::element_not_found("element"))
}

/// Depth-first search for a single element matching `entry`.
///
/// `deadline` is shared across retry attempts so the total wall-clock time is
/// bounded at five seconds.
#[cfg(target_os = "macos")]
pub fn find_element_recursive(
    el: &AXElement,
    entry: &RefEntry,
    depth: u8,
    max_depth: u8,
    ancestors: &mut FxHashSet<usize>,
    deadline: std::time::Instant,
) -> Result<NativeHandle, AdapterError> {
    use accessibility_sys::kAXRoleAttribute;
    use core_foundation::base::{CFRetain, CFTypeRef};

    if std::time::Instant::now() > deadline {
        return Err(
            AdapterError::new(ErrorCode::Timeout, "Element resolution timed out")
                .with_suggestion("Retry the command, or run 'snapshot' if the UI changed."),
        );
    }

    let ptr_key = el.0 as usize;
    if !ancestors.insert(ptr_key) {
        return Err(AdapterError::element_not_found("element"));
    }

    let ax_role = copy_string_attr(el, kAXRoleAttribute);
    let normalized = crate::tree::roles::normalized_role_for_element(el, ax_role.as_deref());

    if normalized == entry.role && element_matches_entry(el, entry) {
        ancestors.remove(&ptr_key);
        unsafe { CFRetain(el.0 as CFTypeRef) };
        return Ok(unsafe { NativeHandle::from_ptr(el.0 as *const _) });
    }

    if depth >= max_depth {
        ancestors.remove(&ptr_key);
        return Err(AdapterError::element_not_found("element"));
    }

    if should_prune_by_bounds(el, entry, depth) {
        ancestors.remove(&ptr_key);
        return Err(AdapterError::element_not_found("element"));
    }

    let children = resolve_children(el, ax_role.as_deref());

    for child in &children {
        if let Ok(handle) =
            find_element_recursive(child, entry, depth + 1, max_depth, ancestors, deadline)
        {
            ancestors.remove(&ptr_key);
            return Ok(handle);
        }
    }

    ancestors.remove(&ptr_key);
    Err(AdapterError::element_not_found("element"))
}

#[cfg(target_os = "macos")]
fn identity_matches(
    entry: &RefEntry,
    actual_name: Option<&str>,
    actual_value: Option<&str>,
    actual_description: Option<&str>,
) -> bool {
    let expected = [
        meaningful_text(entry.name.as_deref()),
        meaningful_text(entry.value.as_deref()),
        meaningful_text(entry.description.as_deref()),
    ];
    let actual = [
        meaningful_text(actual_name),
        meaningful_text(actual_value),
        meaningful_text(actual_description),
    ];

    if expected.iter().all(Option::is_none) {
        return actual.iter().all(Option::is_none);
    }

    expected
        .iter()
        .flatten()
        .any(|expected| actual.iter().flatten().any(|actual| actual == expected))
}

#[cfg(target_os = "macos")]
fn meaningful_text(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
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
fn bounds_match(el: &AXElement, entry: &RefEntry) -> bool {
    match entry.bounds_hash {
        Some(expected) => {
            let actual = crate::tree::read_bounds(el).map(|b| b.bounds_hash());
            actual.map(|h| h == expected).unwrap_or(false)
        }
        None => true,
    }
}

#[cfg(target_os = "macos")]
fn should_prune_by_bounds(el: &AXElement, entry: &RefEntry, depth: u8) -> bool {
    if depth == 0 || entry.bounds.is_none() || entry.bounds_hash.is_none() {
        return false;
    }
    let Some(candidate) = crate::tree::read_bounds(el) else {
        return false;
    };
    let Some(target) = entry.bounds.as_ref() else {
        return false;
    };
    !rects_overlap(&candidate, target)
}

#[cfg(target_os = "macos")]
fn rects_overlap(
    candidate: &agent_desktop_core::node::Rect,
    target: &agent_desktop_core::node::Rect,
) -> bool {
    let candidate_right = candidate.x + candidate.width;
    let candidate_bottom = candidate.y + candidate.height;
    let target_right = target.x + target.width;
    let target_bottom = target.y + target.height;
    candidate.x <= target_right
        && candidate_right >= target.x
        && candidate.y <= target_bottom
        && candidate_bottom >= target.y
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

#[cfg(not(target_os = "macos"))]
pub fn find_element_recursive(
    _el: &AXElement,
    _entry: &RefEntry,
    _depth: u8,
    _max_depth: u8,
    _ancestors: &mut FxHashSet<usize>,
    _deadline: std::time::Instant,
) -> Result<NativeHandle, AdapterError> {
    Err(AdapterError::not_supported("find_element_recursive"))
}
