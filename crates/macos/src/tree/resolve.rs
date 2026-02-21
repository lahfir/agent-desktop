use agent_desktop_core::{adapter::NativeHandle, error::AdapterError, refs::RefEntry};
use rustc_hash::FxHashSet;

use super::element::{
    copy_ax_array, copy_string_attr, element_for_pid, resolve_element_name, AXElement,
};

#[cfg(target_os = "macos")]
pub fn resolve_element_impl(entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
    let root = element_for_pid(entry.pid);
    let mut visited = FxHashSet::default();
    find_element_recursive(&root, entry, 0, 20, &mut visited)
}

#[cfg(target_os = "macos")]
pub fn find_element_recursive(
    el: &AXElement,
    entry: &RefEntry,
    depth: u8,
    max_depth: u8,
    ancestors: &mut FxHashSet<usize>,
) -> Result<NativeHandle, AdapterError> {
    use accessibility_sys::kAXRoleAttribute;
    use core_foundation::base::{CFRetain, CFTypeRef};

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

    let child_attr = if ax_role.as_deref() == Some("AXBrowser") {
        "AXColumns"
    } else {
        "AXChildren"
    };
    let children = copy_ax_array(el, child_attr)
        .filter(|v| !v.is_empty())
        .or_else(|| copy_ax_array(el, "AXContents").filter(|v| !v.is_empty()))
        .unwrap_or_default();

    for child in &children {
        if let Ok(handle) = find_element_recursive(child, entry, depth + 1, max_depth, ancestors) {
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
) -> Result<NativeHandle, AdapterError> {
    Err(AdapterError::not_supported("find_element_recursive"))
}
