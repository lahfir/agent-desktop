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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::node::Rect;

    fn node(role: &str, name: Option<&str>, value: Option<&str>) -> AccessibilityNode {
        AccessibilityNode {
            ref_id: None,
            role: role.into(),
            name: name.map(str::to_string),
            value: value.map(str::to_string),
            description: None,
            hint: None,
            states: vec![],
            bounds: None,
            children: vec![],
            children_count: None,
        }
    }

    #[test]
    fn finds_first_matching_role() {
        let mut root = node("window", None, None);
        root.children.push(node("button", Some("OK"), None));
        root.children.push(node("button", Some("Cancel"), None));

        let hit = find_first_match(&root, Some("button"), None, None).unwrap();
        assert_eq!(hit.name.as_deref(), Some("OK"));
    }

    #[test]
    fn value_substring_disambiguates_duplicate_labels() {
        let mut root = node("window", None, None);
        root.children
            .push(node("button", Some("Save"), Some("pressed")));
        root.children
            .push(node("button", Some("Save"), Some("default")));

        let hit = find_first_match(&root, Some("button"), Some("Save"), Some("default")).unwrap();
        assert_eq!(hit.value.as_deref(), Some("default"));
    }

    #[test]
    fn matched_node_bounds_hash_stable_across_calls() {
        let mut matched = node("button", Some("OK"), None);
        matched.bounds = Some(Rect {
            x: 10.0,
            y: 20.0,
            width: 80.0,
            height: 24.0,
        });
        let first = matched.bounds.as_ref().map(|r| r.bounds_hash()).unwrap();
        let second = matched.bounds.as_ref().map(|r| r.bounds_hash()).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn missing_filter_field_treats_as_dont_care() {
        let root = node("button", Some("OK"), Some("enabled"));
        assert!(find_first_match(&root, None, None, None).is_some());
        assert!(find_first_match(&root, Some("button"), None, None).is_some());
        assert!(find_first_match(&root, None, Some("OK"), None).is_some());
    }

    #[test]
    fn no_match_returns_none() {
        let root = node("window", Some("App"), None);
        assert!(find_first_match(&root, Some("slider"), None, None).is_none());
        assert!(find_first_match(&root, None, Some("nonexistent"), None).is_none());
    }
}
