use agent_desktop_core::{
    adapter::NativeHandle,
    error::AdapterError,
    locator::{self, LocatorQuery, NodeMatchContext},
    node::Rect,
};
use core_foundation::base::{CFRetain, CFTypeRef};
use rustc_hash::FxHashSet;

use super::{
    AXElement, NodeAttrs, copy_ax_array,
    element::{element_for_pid, fetch_node_attrs},
    native_id::meaningful_native_id,
    roles,
    state_reader::{self, StateReaderContext},
};
use accessibility_sys::kAXChildrenAttribute;

const DEFAULT_QUERY_DEPTH: u8 = 10;

pub fn resolve_query_impl(
    query: &LocatorQuery,
    pid: i32,
    scope: Option<&NativeHandle>,
) -> Result<Vec<NativeHandle>, AdapterError> {
    query.validate_states()?;
    let root = scope_root(scope, pid)?;
    let mut matches = Vec::new();
    let mut ancestors = FxHashSet::default();
    collect_matches(
        &root,
        query,
        0,
        DEFAULT_QUERY_DEPTH,
        &mut ancestors,
        &mut matches,
        None,
    )?;
    Ok(matches)
}

fn scope_root(scope: Option<&NativeHandle>, pid: i32) -> Result<AXElement, AdapterError> {
    let Some(scope) = scope else {
        return Ok(element_for_pid(pid));
    };
    Ok(scope_element(scope))
}

fn scope_element(handle: &NativeHandle) -> AXElement {
    let el = AXElement(handle.as_raw() as accessibility_sys::AXUIElementRef);
    unsafe { CFRetain(el.0 as CFTypeRef) };
    el
}

fn collect_matches(
    el: &AXElement,
    query: &LocatorQuery,
    depth: u8,
    max_depth: u8,
    ancestors: &mut FxHashSet<usize>,
    matches: &mut Vec<NativeHandle>,
    window_bounds: Option<Rect>,
) -> Result<(), AdapterError> {
    if depth > max_depth {
        return Ok(());
    }
    let ptr_key = el.0 as usize;
    if !ancestors.insert(ptr_key) {
        return Ok(());
    }

    let attrs = fetch_node_attrs(el);
    let role = roles::ax_role_to_str(attrs.role.as_deref().unwrap_or("")).to_string();

    if element_matches(
        el,
        &attrs,
        &role,
        query,
        window_bounds,
        max_depth.saturating_sub(depth),
    )? {
        matches.push(retained_handle(el.clone())?);
    }

    let child_window_bounds = window_bounds_for_children(&attrs, window_bounds);

    if let Some(children) = copy_ax_array(el, kAXChildrenAttribute) {
        for child in &children {
            collect_matches(
                child,
                query,
                depth + 1,
                max_depth,
                ancestors,
                matches,
                child_window_bounds,
            )?;
        }
    }

    ancestors.remove(&ptr_key);
    Ok(())
}

/// Mirrors `builder.rs`'s window-bounds inheritance: a node's own state is
/// computed against the window bounds inherited from its ancestors, but its
/// *children* see the current node's own bounds once that node is itself the
/// window (`AXWindow`). Kept pure and file-local so it stays independently
/// testable without an AX round trip.
fn window_bounds_for_children(attrs: &NodeAttrs, inherited: Option<Rect>) -> Option<Rect> {
    if attrs.role.as_deref() == Some("AXWindow") {
        attrs.bounds.or(inherited)
    } else {
        inherited
    }
}

fn element_matches(
    el: &AXElement,
    attrs: &NodeAttrs,
    role: &str,
    query: &LocatorQuery,
    window_bounds: Option<Rect>,
    remaining_depth: u8,
) -> Result<bool, AdapterError> {
    if !locator::role_matches(query, role) {
        return Ok(false);
    }
    let state_ctx = StateReaderContext {
        focused: None,
        window_bounds,
        is_secure_text: attrs.role.as_deref() == Some("AXSecureTextField"),
    };
    let states = state_reader::states_from_element(el, attrs, role, &state_ctx);
    let children = if query.containment.has.is_some() || query.containment.has_not.is_some() {
        build_child_nodes(el, remaining_depth, window_bounds)
    } else {
        Vec::new()
    };
    let native_id = meaningful_native_id(attrs.native_id.clone());
    let ctx = NodeMatchContext {
        role,
        name: attrs.title.as_deref(),
        description: attrs.description.as_deref(),
        native_id: native_id.as_deref(),
        value: attrs.value.as_deref(),
        states: &states,
        children: &children,
    };
    Ok(locator::node_matches(query, ctx))
}

fn build_child_nodes(
    el: &AXElement,
    max_depth: u8,
    window_bounds: Option<Rect>,
) -> Vec<agent_desktop_core::node::AccessibilityNode> {
    let Some(children) = copy_ax_array(el, kAXChildrenAttribute) else {
        return Vec::new();
    };
    children
        .iter()
        .filter_map(|child| ax_node_shallow(child, max_depth.saturating_sub(1), window_bounds))
        .collect()
}

fn ax_node_shallow(
    el: &AXElement,
    remaining_depth: u8,
    window_bounds: Option<Rect>,
) -> Option<agent_desktop_core::node::AccessibilityNode> {
    let attrs = fetch_node_attrs(el);
    let role = roles::ax_role_to_str(attrs.role.as_deref().unwrap_or("")).to_string();
    let state_ctx = StateReaderContext {
        focused: None,
        window_bounds,
        is_secure_text: false,
    };
    let states = state_reader::states_from_element(el, &attrs, &role, &state_ctx);
    let children = if remaining_depth > 0 {
        let child_window_bounds = window_bounds_for_children(&attrs, window_bounds);
        build_child_nodes(el, remaining_depth, child_window_bounds)
    } else {
        Vec::new()
    };
    Some(agent_desktop_core::node::AccessibilityNode {
        ref_id: None,
        role,
        name: attrs.title.clone(),
        value: attrs.value.clone(),
        description: attrs.description.clone(),
        native_id: meaningful_native_id(attrs.native_id.clone()),
        hint: None,
        states,
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children,
    })
}

fn retained_handle(candidate: AXElement) -> Result<NativeHandle, AdapterError> {
    if candidate.0.is_null() {
        return Err(AdapterError::element_not_found("element"));
    }
    unsafe { CFRetain(candidate.0 as CFTypeRef) };
    Ok(unsafe { NativeHandle::from_ptr(candidate.0 as *const _) })
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
