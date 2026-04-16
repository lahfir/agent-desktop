use agent_desktop_core::node::AccessibilityNode;

/// Finds the first node in DFS order that matches every provided filter.
/// Filters are ANDed; a `None` filter means "don't care". Substring
/// matching is case-insensitive to tolerate platform-specific casing
/// (macOS AX strings vary between "AXButton" and "button").
pub(crate) fn find_first_match<'a>(
    node: &'a AccessibilityNode,
    role: Option<&str>,
    name: Option<&str>,
    value: Option<&str>,
) -> Option<&'a AccessibilityNode> {
    if matches_all(node, role, name, value) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(hit) = find_first_match(child, role, name, value) {
            return Some(hit);
        }
    }
    None
}

fn matches_all(
    node: &AccessibilityNode,
    role: Option<&str>,
    name: Option<&str>,
    value: Option<&str>,
) -> bool {
    if let Some(r) = role {
        if !contains_ignore_case(&node.role, r) {
            return false;
        }
    }
    if let Some(n) = name {
        match node.name.as_deref() {
            Some(actual) if contains_ignore_case(actual, n) => {}
            _ => return false,
        }
    }
    if let Some(v) = value {
        match node.value.as_deref() {
            Some(actual) if contains_ignore_case(actual, v) => {}
            _ => return false,
        }
    }
    true
}

fn contains_ignore_case(haystack: &str, needle: &str) -> bool {
    haystack.to_lowercase().contains(&needle.to_lowercase())
}
