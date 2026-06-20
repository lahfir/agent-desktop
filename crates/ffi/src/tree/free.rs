use crate::convert::string::free_c_string;
use crate::types::{AdNode, AdNodeTree};
use std::os::raw::c_char;
use std::ptr;

const MAX_NODE_STATE_STRINGS_TO_FREE: usize = 1024;
const MAX_TREE_NODES_TO_FREE: usize = 1_000_000;

unsafe fn free_c_string_array(arr: *mut *mut c_char) {
    unsafe {
        if arr.is_null() {
            return;
        }
        let mut len = 0;
        while len < MAX_NODE_STATE_STRINGS_TO_FREE && !(*arr.add(len)).is_null() {
            free_c_string(*arr.add(len));
            len += 1;
        }
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            arr,
            len + 1,
        )));
    }
}

unsafe fn free_node_fields(node: &mut AdNode) {
    unsafe {
        free_c_string(node.ref_id as *mut c_char);
        free_c_string(node.role as *mut c_char);
        free_c_string(node.name as *mut c_char);
        free_c_string(node.value as *mut c_char);
        free_c_string(node.description as *mut c_char);
        free_c_string(node.hint as *mut c_char);
        free_c_string_array(node.states);
        node.ref_id = ptr::null();
        node.role = ptr::null();
        node.name = ptr::null();
        node.value = ptr::null();
        node.description = ptr::null();
        node.hint = ptr::null();
        node.states = ptr::null_mut();
        node.state_count = 0;
    }
}

/// # Safety
/// `tree` must be null or point to a valid `AdNodeTree` previously returned
/// by `flatten_tree` or `ad_get_tree`. After this call the tree is zeroed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_free_tree(tree: *mut AdNodeTree) {
    crate::ffi_try::trap_panic_void(|| unsafe {
        if tree.is_null() {
            return;
        }
        let tree = &mut *tree;
        if tree.nodes.is_null() {
            return;
        }
        let node_count = sentinel_node_count(tree.nodes);
        let nodes = std::slice::from_raw_parts_mut(tree.nodes, node_count);
        for node in nodes.iter_mut() {
            free_node_fields(node);
        }
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            tree.nodes,
            node_count + 1,
        )));
        tree.nodes = ptr::null_mut();
        tree.count = 0;
    })
}

unsafe fn sentinel_node_count(nodes: *mut AdNode) -> usize {
    unsafe {
        let mut count = 0;
        while count < MAX_TREE_NODES_TO_FREE && !(*nodes.add(count)).role.is_null() {
            count += 1;
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_free_null_tree_is_noop() {
        unsafe { ad_free_tree(std::ptr::null_mut()) };
    }

    #[test]
    fn free_tree_ignores_mutated_node_state_count() {
        let mut tree = tree_with_node(node_with_states(&["focused"], u32::MAX));
        unsafe { ad_free_tree(&mut tree) };

        assert!(tree.nodes.is_null());
    }

    fn tree_with_node(node: AdNode) -> AdNodeTree {
        let mut nodes = vec![node, sentinel_node()].into_boxed_slice();
        let raw = nodes.as_mut_ptr();
        std::mem::forget(nodes);
        AdNodeTree {
            nodes: raw,
            count: 1,
        }
    }

    fn node_with_states(states: &[&str], state_count: u32) -> AdNode {
        AdNode {
            ref_id: ptr::null(),
            role: crate::convert::string::string_to_c_lossy("button"),
            name: ptr::null(),
            value: ptr::null(),
            description: ptr::null(),
            hint: ptr::null(),
            states: state_array(states),
            state_count,
            bounds: crate::types::AdRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            has_bounds: false,
            parent_index: -1,
            child_start: 0,
            child_count: 0,
        }
    }

    fn sentinel_node() -> AdNode {
        AdNode {
            ref_id: ptr::null(),
            role: ptr::null(),
            name: ptr::null(),
            value: ptr::null(),
            description: ptr::null(),
            hint: ptr::null(),
            states: ptr::null_mut(),
            state_count: 0,
            bounds: crate::types::AdRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            has_bounds: false,
            parent_index: -1,
            child_start: 0,
            child_count: 0,
        }
    }

    fn state_array(states: &[&str]) -> *mut *mut c_char {
        let mut ptrs: Vec<*mut c_char> = states
            .iter()
            .map(|state| crate::convert::string::string_to_c_lossy(state))
            .collect();
        ptrs.push(ptr::null_mut());
        let mut boxed = ptrs.into_boxed_slice();
        let raw = boxed.as_mut_ptr();
        std::mem::forget(boxed);
        raw
    }

    #[test]
    fn free_tree_ignores_mutated_tree_count() {
        let root = agent_desktop_core::node::AccessibilityNode {
            ref_id: None,
            role: "button".into(),
            name: None,
            value: None,
            description: None,
            hint: None,
            states: vec![],
            available_actions: vec![],
            bounds: None,
            children: vec![],
            children_count: None,
        };
        let mut tree = crate::tree::flatten::flatten_tree(&root);
        tree.count = u32::MAX;
        unsafe { ad_free_tree(&mut tree) };

        assert!(tree.nodes.is_null());
        assert_eq!(tree.count, 0);
    }
}
