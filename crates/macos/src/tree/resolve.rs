use agent_desktop_core::{adapter::NativeHandle, error::AdapterError, refs::RefEntry};
use rustc_hash::FxHashSet;

use super::element::{
    child_attributes, copy_ax_array, copy_string_attr, element_for_pid, resolve_element_name,
    AXElement,
};

#[cfg(target_os = "macos")]
pub fn resolve_element_impl(entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    tracing::debug!(
        "resolve: searching pid={} role={} name={:?} bounds_hash={:?}",
        entry.pid,
        entry.role,
        entry.name.as_deref().unwrap_or("(none)"),
        entry.bounds_hash
    );
    let root = element_for_pid(entry.pid);
    let mut visited = FxHashSet::default();
    let resolve_depth: u8 = 50;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    if let Ok(handle) =
        find_element_recursive(&root, entry, 0, resolve_depth, &mut visited, deadline)
    {
        tracing::debug!("resolve: found exact match");
        return Ok(handle);
    }
    if entry.bounds_hash.is_some() && entry.name.is_some() {
        tracing::debug!("resolve: exact match failed, trying relaxed (name-only)");
        let relaxed = RefEntry {
            bounds_hash: None,
            ..entry.clone()
        };
        visited.clear();
        if let Ok(handle) =
            find_element_recursive(&root, &relaxed, 0, resolve_depth, &mut visited, deadline)
        {
            tracing::debug!("resolve: found via relaxed match (bounds changed)");
            return Ok(handle);
        }
    }
    tracing::debug!("resolve: element not found");
    Err(AdapterError::new(
        agent_desktop_core::error::ErrorCode::StaleRef,
        format!(
            "Element not found: role={}, name={:?}",
            entry.role,
            entry.name.as_deref().unwrap_or("(none)")
        ),
    )
    .with_suggestion("Run 'snapshot' to refresh, then retry with the updated ref."))
}

/// Depth-first search for a single element matching `entry`.
///
/// `deadline` is shared across the full resolve call (exact + relaxed passes)
/// so the total wall-clock time is bounded at five seconds. Subtrees whose
/// spatial bounds do not contain the target's centre point are pruned, which
/// makes resolution over large documents (e.g. dense spreadsheets) fast even
/// when the AX tree has thousands of nodes.
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
        return Err(AdapterError::new(
            agent_desktop_core::error::ErrorCode::StaleRef,
            "Element resolution timed out",
        )
        .with_suggestion("Run 'snapshot' to refresh, then retry with the updated ref."));
    }

    let ptr_key = el.0 as usize;
    if !ancestors.insert(ptr_key) {
        return Err(AdapterError::element_not_found("element"));
    }

    let ax_role = copy_string_attr(el, kAXRoleAttribute);
    let normalized = ax_role
        .as_deref()
        .map(crate::tree::roles::ax_role_to_str)
        .unwrap_or("unknown");

    if normalized == entry.role {
        let elem_name = resolve_element_name(el);
        let name_match = match (&entry.name, &elem_name) {
            (Some(en), Some(nn)) => en == nn,
            (None, None) => true,
            _ => false,
        };
        let bounds_match = match entry.bounds_hash {
            Some(expected) => {
                let actual = crate::tree::read_bounds(el).map(|b| b.bounds_hash());
                actual.map(|h| h == expected).unwrap_or(false)
            }
            None => true,
        };
        if name_match && bounds_match {
            ancestors.remove(&ptr_key);
            unsafe { CFRetain(el.0 as CFTypeRef) };
            return Ok(NativeHandle::from_ptr(el.0 as *const _));
        }
    }

    if depth >= max_depth {
        ancestors.remove(&ptr_key);
        return Err(AdapterError::element_not_found("element"));
    }

    if let Some(target) = &entry.bounds {
        if let Some(el_bounds) = crate::tree::read_bounds(el) {
            if el_bounds.width > 0.0 && el_bounds.height > 0.0 {
                let cx = target.x + target.width / 2.0;
                let cy = target.y + target.height / 2.0;
                if cx < el_bounds.x
                    || cx > el_bounds.x + el_bounds.width
                    || cy < el_bounds.y
                    || cy > el_bounds.y + el_bounds.height
                {
                    ancestors.remove(&ptr_key);
                    return Err(AdapterError::element_not_found("element"));
                }
            }
        }
    }

    let children = child_attributes(ax_role.as_deref())
        .iter()
        .find_map(|attr| copy_ax_array(el, attr).filter(|v| !v.is_empty()))
        .unwrap_or_default();

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
