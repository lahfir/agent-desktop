use super::*;
use crate::{node::AccessibilityNode, search_text};

fn node(role: &str, name: Option<&str>, value: Option<&str>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        name: name.map(String::from),
        value: value.map(String::from),
        description: None,
        native_id: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![],
    }
}

#[test]
fn parse_selector_role_and_text() {
    let query = parse_selector("button:Submit");
    assert_eq!(query.role.as_deref(), Some("button"));
    assert_eq!(query.has_text.as_deref(), Some("submit"));
}

#[test]
fn parse_selector_role_only() {
    let query = parse_selector("button");
    assert_eq!(query.role.as_deref(), Some("button"));
    assert!(query.has_text.is_none());
}

#[test]
fn parse_selector_text_only() {
    let query = parse_selector(":Saved!");
    assert!(query.role.is_none());
    assert_eq!(query.has_text.as_deref(), Some("saved!"));
}

#[test]
fn parse_selector_match_everything_variants() {
    for raw in ["", ":", " : "] {
        let query = parse_selector(raw);
        assert!(query.is_empty(), "expected empty query for {raw:?}");
    }
}

#[test]
fn parse_selector_empty_text_side_becomes_none() {
    let query = parse_selector("button:");
    assert_eq!(query.role.as_deref(), Some("button"));
    assert!(query.has_text.is_none());
}

#[test]
fn parse_selector_trims_whitespace() {
    let query = parse_selector(" button : Submit ");
    assert_eq!(query.role.as_deref(), Some("button"));
    assert_eq!(query.has_text.as_deref(), Some("submit"));
}

#[test]
fn parse_selector_splits_on_first_colon_only() {
    let query = parse_selector("textfield:a:b");
    assert_eq!(query.role.as_deref(), Some("textfield"));
    assert_eq!(query.has_text.as_deref(), Some("a:b"));
}

#[test]
fn find_query_without_text_never_calls_contains_with_empty_needle() {
    let root = node("button", Some("Save"), None);
    let query = FindQuery {
        role: Some("button".into()),
        ..FindQuery::default()
    };
    assert!(tree_has_match(&root, &query));
}

#[test]
fn tree_has_match_finds_nested_node() {
    let mut root = node("window", None, None);
    root.children.push(node("button", Some("Submit"), None));
    let query = FindQuery {
        role: Some("button".into()),
        has_text: Some(search_text::normalize("submit")),
        ..FindQuery::default()
    };
    assert!(tree_has_match(&root, &query));
}
