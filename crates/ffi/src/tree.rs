use crate::convert::{free_c_string, opt_string_to_c, string_to_c};
use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::types::{AdNode, AdNodeTree, AdRect, AdTreeOptions};
use crate::AdAdapter;
use agent_desktop_core::node::AccessibilityNode;
use std::os::raw::c_char;
use std::ptr;

#[allow(dead_code)]
pub(crate) fn flatten_tree(root: &AccessibilityNode) -> AdNodeTree {
    let mut flat: Vec<AdNode> = Vec::new();
    flatten_recursive(root, -1, &mut flat);
    let count = flat.len() as u32;
    let nodes = if flat.is_empty() {
        ptr::null_mut()
    } else {
        let mut boxed = flat.into_boxed_slice();
        let ptr = boxed.as_mut_ptr();
        std::mem::forget(boxed);
        ptr
    };
    AdNodeTree { nodes, count }
}

#[allow(dead_code)]
fn flatten_recursive(node: &AccessibilityNode, parent_index: i32, flat: &mut Vec<AdNode>) {
    let my_index = flat.len() as i32;

    let (states_ptr, state_count) = strings_to_c_array(&node.states);
    let (bounds, has_bounds) = match &node.bounds {
        Some(r) => (crate::convert::rect_to_c(r), true),
        None => (
            AdRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            false,
        ),
    };

    // Push with placeholder child_start — filled in after we know where children go
    flat.push(AdNode {
        ref_id: opt_string_to_c(node.ref_id.as_deref()),
        role: string_to_c(&node.role),
        name: opt_string_to_c(node.name.as_deref()),
        value: opt_string_to_c(node.value.as_deref()),
        description: opt_string_to_c(node.description.as_deref()),
        hint: opt_string_to_c(node.hint.as_deref()),
        states: states_ptr,
        state_count,
        bounds,
        has_bounds,
        parent_index,
        child_start: 0,
        child_count: node.children.len() as u32,
    });

    // Children start right after this node's descendants are placed
    let child_start = flat.len() as u32;
    flat[my_index as usize].child_start = child_start;

    for child in &node.children {
        flatten_recursive(child, my_index, flat);
    }
}

#[allow(dead_code)]
fn strings_to_c_array(strings: &[String]) -> (*mut *mut c_char, u32) {
    if strings.is_empty() {
        return (ptr::null_mut(), 0);
    }
    let ptrs: Vec<*mut c_char> = strings.iter().map(|s| string_to_c(s)).collect();
    let count = ptrs.len() as u32;
    let mut boxed = ptrs.into_boxed_slice();
    let ptr = boxed.as_mut_ptr();
    std::mem::forget(boxed);
    (ptr, count)
}

unsafe fn free_c_string_array(arr: *mut *mut c_char, count: u32) {
    if arr.is_null() {
        return;
    }
    let slice = std::slice::from_raw_parts_mut(arr, count as usize);
    for p in slice.iter_mut() {
        free_c_string(*p);
    }
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        arr,
        count as usize,
    )));
}

unsafe fn free_node_fields(node: &mut AdNode) {
    free_c_string(node.ref_id as *mut c_char);
    free_c_string(node.role as *mut c_char);
    free_c_string(node.name as *mut c_char);
    free_c_string(node.value as *mut c_char);
    free_c_string(node.description as *mut c_char);
    free_c_string(node.hint as *mut c_char);
    free_c_string_array(node.states, node.state_count);
    node.ref_id = ptr::null();
    node.role = ptr::null();
    node.name = ptr::null();
    node.value = ptr::null();
    node.description = ptr::null();
    node.hint = ptr::null();
    node.states = ptr::null_mut();
    node.state_count = 0;
}

/// # Safety
/// `tree` must be null or point to a valid `AdNodeTree` previously returned
/// by `flatten_tree` or `ad_get_tree`. After this call the tree is zeroed.
#[no_mangle]
pub unsafe extern "C" fn ad_free_tree(tree: *mut AdNodeTree) {
    if tree.is_null() {
        return;
    }
    let tree = &mut *tree;
    if tree.nodes.is_null() {
        return;
    }
    let nodes = std::slice::from_raw_parts_mut(tree.nodes, tree.count as usize);
    for node in nodes.iter_mut() {
        free_node_fields(node);
    }
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
        tree.nodes,
        tree.count as usize,
    )));
    tree.nodes = ptr::null_mut();
    tree.count = 0;
}

/// # Safety
/// All pointers must be valid. `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ad_get_tree(
    adapter: *const AdAdapter,
    win: *const crate::types::AdWindowInfo,
    opts: *const AdTreeOptions,
    out: *mut AdNodeTree,
) -> AdResult {
    let adapter = &*adapter;
    let opts_ref = &*opts;
    let core_win = crate::windows::ad_window_to_core(&*win);
    let core_opts = agent_desktop_core::adapter::TreeOptions {
        max_depth: opts_ref.max_depth,
        include_bounds: opts_ref.include_bounds,
        interactive_only: opts_ref.interactive_only,
        compact: opts_ref.compact,
        surface: agent_desktop_core::adapter::SnapshotSurface::Window,
    };

    match adapter.inner.get_tree(&core_win, &core_opts) {
        Ok(tree) => {
            clear_last_error();
            *out = flatten_tree(&tree);
            AdResult::Ok
        }
        Err(e) => {
            set_last_error(&e);
            crate::error::last_error_code()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(role: &str) -> AccessibilityNode {
        AccessibilityNode {
            ref_id: None,
            role: role.into(),
            name: None,
            value: None,
            description: None,
            hint: None,
            states: vec![],
            bounds: None,
            children: vec![],
        }
    }

    #[test]
    fn test_flatten_single_node() {
        let root = node("window");
        let tree = flatten_tree(&root);
        assert_eq!(tree.count, 1);
        let nodes = unsafe { std::slice::from_raw_parts(tree.nodes, 1) };
        assert_eq!(nodes[0].parent_index, -1);
        assert_eq!(nodes[0].child_count, 0);
        let role = unsafe { crate::convert::c_to_str(nodes[0].role) };
        assert_eq!(role, Some("window"));
        unsafe { ad_free_tree(&tree as *const _ as *mut _) };
    }

    #[test]
    fn test_flatten_parent_child() {
        let btn = node("button");
        let mut root = node("window");
        root.children = vec![btn];

        let tree = flatten_tree(&root);
        assert_eq!(tree.count, 2);
        let nodes = unsafe { std::slice::from_raw_parts(tree.nodes, 2) };

        // root
        assert_eq!(nodes[0].parent_index, -1);
        assert_eq!(nodes[0].child_start, 1);
        assert_eq!(nodes[0].child_count, 1);

        // button
        assert_eq!(nodes[1].parent_index, 0);
        assert_eq!(nodes[1].child_count, 0);
        let role = unsafe { crate::convert::c_to_str(nodes[1].role) };
        assert_eq!(role, Some("button"));

        unsafe { ad_free_tree(&tree as *const _ as *mut _) };
    }

    #[test]
    fn test_flatten_depth_first_order() {
        // root -> [a -> [a1, a2], b]
        let a1 = node("a1");
        let a2 = node("a2");
        let mut a = node("a");
        a.children = vec![a1, a2];
        let b = node("b");
        let mut root = node("root");
        root.children = vec![a, b];

        let tree = flatten_tree(&root);
        assert_eq!(tree.count, 5);
        let nodes = unsafe { std::slice::from_raw_parts(tree.nodes, 5) };

        // Depth-first: root(0), a(1), a1(2), a2(3), b(4)
        let roles: Vec<_> = nodes
            .iter()
            .map(|n| unsafe { crate::convert::c_to_str(n.role).unwrap() })
            .collect();
        assert_eq!(roles, vec!["root", "a", "a1", "a2", "b"]);

        // root's children start at 1 (a and b)
        assert_eq!(nodes[0].child_start, 1);
        assert_eq!(nodes[0].child_count, 2);

        // a's children start at 2 (a1 and a2)
        assert_eq!(nodes[1].child_start, 2);
        assert_eq!(nodes[1].child_count, 2);
        assert_eq!(nodes[1].parent_index, 0);

        // b's parent is root (0), no children
        assert_eq!(nodes[4].parent_index, 0);
        assert_eq!(nodes[4].child_count, 0);

        unsafe { ad_free_tree(&tree as *const _ as *mut _) };
    }

    #[test]
    fn test_flatten_with_states() {
        let mut btn = node("button");
        btn.states = vec!["focused".into(), "enabled".into()];
        let tree = flatten_tree(&btn);
        let nodes = unsafe { std::slice::from_raw_parts(tree.nodes, 1) };
        assert_eq!(nodes[0].state_count, 2);
        let states = unsafe { std::slice::from_raw_parts(nodes[0].states, 2) };
        let s0 = unsafe { crate::convert::c_to_str(states[0]) };
        let s1 = unsafe { crate::convert::c_to_str(states[1]) };
        assert_eq!(s0, Some("focused"));
        assert_eq!(s1, Some("enabled"));
        unsafe { ad_free_tree(&tree as *const _ as *mut _) };
    }

    #[test]
    fn test_free_null_tree_is_noop() {
        unsafe { ad_free_tree(std::ptr::null_mut()) };
    }
}
