use crate::convert::string::{opt_string_to_c, string_to_c_lossy};
use crate::types::{AdNode, AdNodeTree, AdRect};
use agent_desktop_core::node::AccessibilityNode;
use std::collections::VecDeque;
use std::os::raw::c_char;
use std::ptr;

/// Flattens an `AccessibilityNode` tree into the BFS-ordered layout
/// C consumers see via `AdNodeTree.nodes`.
///
/// Guarantees:
/// - Direct children of any `AdNode` at index `i` live contiguously at
///   `nodes[n.child_start .. n.child_start + n.child_count]`. This is
///   the BFS (level-order) layout.
/// - `parent_index` is `-1` for the root and otherwise a valid back-index
///   into `nodes`.
/// - `child_count` is zero when `child_start` is not indexable.
///
/// A recursive DFS layout placed node `a1` immediately after its parent
/// `a`, overlapping with `a`'s siblings — the range
/// `a.child_start..a.child_start + a.child_count` therefore stepped into
/// grandchildren. BFS keeps siblings contiguous by construction.
pub(crate) fn flatten_tree(root: &AccessibilityNode) -> AdNodeTree {
    let total = count_nodes(root);
    let mut flat: Vec<AdNode> = Vec::with_capacity(total);

    flat.push(to_ad_node(root, -1));
    let mut queue: VecDeque<(&AccessibilityNode, usize)> = VecDeque::new();
    queue.push_back((root, 0));

    while let Some((node, node_idx)) = queue.pop_front() {
        if node.children.is_empty() {
            continue;
        }
        let child_start = flat.len() as u32;
        let child_count = node.children.len() as u32;
        flat[node_idx].child_start = child_start;
        flat[node_idx].child_count = child_count;
        for child in &node.children {
            let child_idx = flat.len();
            flat.push(to_ad_node(child, node_idx as i32));
            queue.push_back((child, child_idx));
        }
    }

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

fn count_nodes(node: &AccessibilityNode) -> usize {
    let mut total: usize = 0;
    let mut queue: VecDeque<&AccessibilityNode> = VecDeque::new();
    queue.push_back(node);
    while let Some(n) = queue.pop_front() {
        total += 1;
        for c in &n.children {
            queue.push_back(c);
        }
    }
    total
}

fn to_ad_node(node: &AccessibilityNode, parent_index: i32) -> AdNode {
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
    AdNode {
        ref_id: opt_string_to_c(node.ref_id.as_deref()),
        role: string_to_c_lossy(&node.role),
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
        child_count: 0,
    }
}

fn strings_to_c_array(strings: &[String]) -> (*mut *mut c_char, u32) {
    if strings.is_empty() {
        return (ptr::null_mut(), 0);
    }
    let ptrs: Vec<*mut c_char> = strings.iter().map(|s| string_to_c_lossy(s)).collect();
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
            children_count: None,
        }
    }

    fn direct_children(nodes: &[AdNode], idx: usize) -> Vec<&AdNode> {
        let n = &nodes[idx];
        let start = n.child_start as usize;
        let end = start + n.child_count as usize;
        nodes[start..end].iter().collect()
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
    fn test_flatten_breadth_first_layout() {
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
        assert_eq!(roles, vec!["root", "a", "b", "a1", "a2"]);

        let root_children: Vec<String> = direct_children(nodes, 0)
            .iter()
            .map(|n| unsafe { c_to_string(n.role).unwrap() })
            .collect();
        assert_eq!(root_children, vec!["a", "b"]);

        let a_idx = nodes
            .iter()
            .position(|n| unsafe { c_to_string(n.role).unwrap() } == "a")
            .unwrap();
        let a_children: Vec<String> = direct_children(nodes, a_idx)
            .iter()
            .map(|n| unsafe { c_to_string(n.role).unwrap() })
            .collect();
        assert_eq!(a_children, vec!["a1", "a2"]);

        let b_idx = nodes
            .iter()
            .position(|n| unsafe { c_to_string(n.role).unwrap() } == "b")
            .unwrap();
        assert!(direct_children(nodes, b_idx).is_empty());

        unsafe { ad_free_tree(&tree as *const _ as *mut _) };
    }

    #[test]
    fn test_flatten_deep_chain() {
        let mut leaf = node("l10");
        for i in (0..10).rev() {
            let mut parent = node(&format!("l{}", i));
            parent.children = vec![leaf];
            leaf = parent;
        }
        let tree = flatten_tree(&leaf);
        assert_eq!(tree.count, 11);
        let nodes = unsafe { std::slice::from_raw_parts(tree.nodes, 11) };

        let mut cursor = 0usize;
        for expected in 0..11 {
            let role = unsafe { c_to_string(nodes[cursor].role).unwrap() };
            assert_eq!(role, format!("l{}", expected));
            let children = direct_children(nodes, cursor);
            if expected < 10 {
                assert_eq!(children.len(), 1);
                cursor = nodes[cursor].child_start as usize;
            } else {
                assert!(children.is_empty());
            }
        }
        unsafe { ad_free_tree(&tree as *const _ as *mut _) };
    }

    #[test]
    fn test_flatten_wide_root() {
        let mut root = node("root");
        for i in 0..100 {
            root.children.push(node(&format!("child_{}", i)));
        }
        let tree = flatten_tree(&root);
        assert_eq!(tree.count, 101);
        let nodes = unsafe { std::slice::from_raw_parts(tree.nodes, 101) };
        let children = direct_children(nodes, 0);
        assert_eq!(children.len(), 100);
        for (i, c) in children.iter().enumerate() {
            let role = unsafe { c_to_string(c.role).unwrap() };
            assert_eq!(role, format!("child_{}", i));
        }
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
