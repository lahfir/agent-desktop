use crate::{node::AccessibilityNode, roles, search_text};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FindQuery {
    pub role: Option<String>,
    pub name: Option<String>,
    pub value: Option<String>,
    pub text: Option<String>,
}

impl FindQuery {
    pub fn is_match_everything(&self) -> bool {
        self.role.is_none() && self.name.is_none() && self.value.is_none() && self.text.is_none()
    }
}

pub fn parse_selector(raw: &str) -> FindQuery {
    let (role_part, text_part) = match raw.split_once(':') {
        Some((left, right)) => (Some(left.trim()), Some(right.trim())),
        None => (Some(raw.trim()), None),
    };

    let role = role_part
        .filter(|part| !part.is_empty())
        .map(roles::normalize_role_query);
    let text = text_part
        .filter(|part| !part.is_empty())
        .map(search_text::normalize);

    FindQuery {
        role,
        name: None,
        value: None,
        text,
    }
}

pub fn node_matches(node: &AccessibilityNode, query: &FindQuery) -> bool {
    let role_match = query.role.as_deref().is_none_or(|r| node.role == r);
    let name_match = query.name.as_deref().is_none_or(|n| {
        node.name
            .as_deref()
            .is_some_and(|text| search_text::contains(text, n))
    });
    let value_match = query.value.as_deref().is_none_or(|v| {
        node.value
            .as_deref()
            .is_some_and(|val| search_text::contains(val, v))
    });
    let text_match = query
        .text
        .as_deref()
        .is_none_or(|t| search_text::node_contains(node, t));
    role_match && name_match && value_match && text_match
}

pub fn tree_has_match(tree: &AccessibilityNode, query: &FindQuery) -> bool {
    if node_matches(tree, query) {
        return true;
    }
    tree.children
        .iter()
        .any(|child| tree_has_match(child, query))
}

#[cfg(test)]
#[path = "query_tests.rs"]
mod tests;
