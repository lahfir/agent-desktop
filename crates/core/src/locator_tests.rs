use super::*;
use crate::{node::AccessibilityNode, search_text, state};

fn node(
    role: &str,
    name: Option<&str>,
    value: Option<&str>,
    states: Vec<&str>,
) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        name: name.map(String::from),
        value: value.map(String::from),
        description: None,
        native_id: None,
        hint: None,
        states: states.into_iter().map(String::from).collect(),
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![],
    }
}

#[test]
fn role_and_name_substring_match() {
    let query = LocatorQuery {
        role: Some("button".into()),
        name: Some(search_text::normalize("save")),
        ..LocatorQuery::default()
    };
    let target = node("button", Some("Save Changes"), None, vec![]);
    assert!(accessibility_node_matches(&target, &query));
}

#[test]
fn exact_name_requires_equality() {
    let query = LocatorQuery {
        name: Some(search_text::normalize("save")),
        exact: true,
        ..LocatorQuery::default()
    };
    assert!(!accessibility_node_matches(
        &node("button", Some("Save Changes"), None, vec![]),
        &query
    ));
    assert!(accessibility_node_matches(
        &node("button", Some("Save"), None, vec![]),
        &query
    ));
}

#[test]
fn state_predicate_filters_checked() {
    let query = LocatorQuery {
        states: vec![StatePredicate {
            token: state::CHECKED.into(),
            expected: Some(true),
        }],
        ..LocatorQuery::default()
    };
    assert!(accessibility_node_matches(
        &node("checkbox", None, None, vec![state::CHECKED]),
        &query
    ));
    assert!(!accessibility_node_matches(
        &node("checkbox", None, None, vec![]),
        &query
    ));
}

#[test]
fn native_id_exact_match() {
    let mut target = node("button", Some("X"), None, vec![]);
    target.native_id = Some("submit-btn".into());
    let query = LocatorQuery {
        native_id: Some("submit-btn".into()),
        ..LocatorQuery::default()
    };
    assert!(accessibility_node_matches(&target, &query));
}

#[test]
fn has_subquery_matches_descendant() {
    let mut root = node("group", None, None, vec![]);
    root.children
        .push(node("statictext", Some("Hello"), None, vec![]));
    let query = LocatorQuery {
        has: Some(Box::new(LocatorQuery {
            has_text: Some(search_text::normalize("hello")),
            ..LocatorQuery::default()
        })),
        ..LocatorQuery::default()
    };
    assert!(accessibility_node_matches(&root, &query));
}

#[test]
fn invalid_state_token_rejected() {
    let query = LocatorQuery {
        states: vec![StatePredicate {
            token: "bogus".into(),
            expected: None,
        }],
        ..LocatorQuery::default()
    };
    assert!(query.validate_states().is_err());
}

#[test]
fn has_not_excludes_subtree_match() {
    let mut root = node("group", None, None, vec![]);
    root.children
        .push(node("button", Some("Delete"), None, vec![]));
    let query = LocatorQuery {
        role: Some("group".into()),
        has_not: Some(Box::new(LocatorQuery {
            role: Some("button".into()),
            name: Some(search_text::normalize("delete")),
            ..LocatorQuery::default()
        })),
        ..LocatorQuery::default()
    };
    assert!(!accessibility_node_matches(&root, &query));
}
