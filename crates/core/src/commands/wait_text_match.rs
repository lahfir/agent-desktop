use crate::{node::AccessibilityNode, search_text};

pub(crate) struct TextMatch {
    pub ref_id: Option<String>,
    pub role: String,
}

pub(crate) fn find_all(node: &AccessibilityNode, text_lower: &str) -> Vec<TextMatch> {
    let mut matches = Vec::new();
    collect(node, text_lower, &mut matches);
    matches
}

fn collect(node: &AccessibilityNode, text_lower: &str, matches: &mut Vec<TextMatch>) {
    if search_text::node_contains(node, text_lower) {
        matches.push(TextMatch {
            ref_id: node.ref_id.clone(),
            role: node.role.clone(),
        });
    }

    for child in &node.children {
        collect(child, text_lower, matches);
    }
}
