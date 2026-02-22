use crate::node::AccessibilityNode;

pub fn add_structural_hints(node: &mut AccessibilityNode) {
    if node.role == "splitter" && node.children.len() > 1 {
        let total = node.children.len();
        for (i, child) in node.children.iter_mut().enumerate() {
            child.hint = Some(format!("column {} of {}", i + 1, total));
        }
    }

    for child in &mut node.children {
        add_structural_hints(child);
    }
}
