use crate::convert::string::{opt_string_to_c, string_to_c};
use crate::types::{AdNode, AdNodeTree, AdRect};
use agent_desktop_core::node::AccessibilityNode;
use std::os::raw::c_char;
use std::ptr;

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

    let child_start = flat.len() as u32;
    flat[my_index as usize].child_start = child_start;

    for child in &node.children {
        flatten_recursive(child, my_index, flat);
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::string::c_to_string;
    use crate::tree::free::ad_free_tree;

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
        let role = unsafe { c_to_string(nodes[0].role) };
        assert_eq!(role.as_deref(), Some("window"));
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

        assert_eq!(nodes[0].parent_index, -1);
        assert_eq!(nodes[0].child_start, 1);
        assert_eq!(nodes[0].child_count, 1);

        assert_eq!(nodes[1].parent_index, 0);
        assert_eq!(nodes[1].child_count, 0);
        let role = unsafe { c_to_string(nodes[1].role) };
        assert_eq!(role.as_deref(), Some("button"));

        unsafe { ad_free_tree(&tree as *const _ as *mut _) };
    }

    #[test]
    fn test_flatten_depth_first_order() {
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

        let roles: Vec<String> = nodes
            .iter()
            .map(|n| unsafe { c_to_string(n.role).unwrap() })
            .collect();
        assert_eq!(roles, vec!["root", "a", "a1", "a2", "b"]);

        assert_eq!(nodes[0].child_start, 1);
        assert_eq!(nodes[0].child_count, 2);

        assert_eq!(nodes[1].child_start, 2);
        assert_eq!(nodes[1].child_count, 2);
        assert_eq!(nodes[1].parent_index, 0);

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
        let s0 = unsafe { c_to_string(states[0]) };
        let s1 = unsafe { c_to_string(states[1]) };
        assert_eq!(s0.as_deref(), Some("focused"));
        assert_eq!(s1.as_deref(), Some("enabled"));
        unsafe { ad_free_tree(&tree as *const _ as *mut _) };
    }
}
