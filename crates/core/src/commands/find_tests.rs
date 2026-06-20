use super::*;

fn node(name: Option<&str>, value: Option<&str>, description: Option<&str>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: Some("@e1".into()),
        role: "textfield".into(),
        name: name.map(String::from),
        value: value.map(String::from),
        description: description.map(String::from),
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![],
    }
}

#[test]
fn display_name_prefers_value_before_description() {
    let root = node(None, Some("current value"), Some("help text"));
    let query = FindQuery {
        role: None,
        name: None,
        value: None,
        text: None,
    };
    let mut matches = Vec::new();

    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);

    assert_eq!(matches[0]["name"], "current value");
}

#[test]
fn search_tree_match_uses_ref_id_contract_and_includes_states() {
    let mut root = node(Some("Save"), None, None);
    root.states = vec!["enabled".into()];
    let query = FindQuery {
        role: None,
        name: None,
        value: None,
        text: None,
    };
    let mut matches = Vec::new();

    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);

    assert_eq!(matches[0]["ref_id"], "@e1");
    assert!(matches[0].get("ref").is_none());
    assert_eq!(matches[0]["states"], serde_json::json!(["enabled"]));
}

#[test]
fn search_tree_matches_text_across_fields() {
    let root = node(None, Some("Primary"), Some("Secondary"));
    let query = FindQuery {
        role: None,
        name: None,
        value: None,
        text: Some(search_text::normalize("secondary")),
    };
    let mut matches = Vec::new();

    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);

    assert_eq!(matches.len(), 1);
}

#[test]
fn default_limit_caps_materialized_matches() {
    let root = AccessibilityNode {
        ref_id: None,
        role: "window".into(),
        name: None,
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: (0..60)
            .map(|i| node(Some(&format!("Button {i}")), None, None))
            .collect(),
    };
    let query = FindQuery {
        role: None,
        name: None,
        value: None,
        text: Some(search_text::normalize("button")),
    };
    let mut matches = Vec::new();

    search_tree(
        &root,
        &query,
        &mut Vec::new(),
        &mut matches,
        Some(DEFAULT_LIMIT),
    );

    assert_eq!(matches.len(), DEFAULT_LIMIT);
}

#[test]
fn limit_conflicts_with_single_result_modes_for_batch_too() {
    let err = validate_find_mode(&FindArgs {
        app: None,
        role: None,
        name: None,
        value: None,
        text: None,
        count: false,
        first: true,
        last: false,
        nth: None,
        limit: Some(10),
    })
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn count_matches_does_not_build_result_json() {
    let root = AccessibilityNode {
        ref_id: None,
        role: "window".into(),
        name: None,
        value: None,
        description: None,
        hint: None,
        states: vec![],
        available_actions: vec![],
        bounds: None,
        children_count: None,
        children: vec![
            node(Some("Save"), None, None),
            node(Some("Cancel"), None, None),
        ],
    };
    let query = FindQuery {
        role: None,
        name: None,
        value: None,
        text: Some(search_text::normalize("a")),
    };

    assert_eq!(count_matches(&root, &query), 2);
}

fn role_node(role: &str, name: Option<&str>) -> AccessibilityNode {
    let mut n = node(name, None, None);
    n.role = role.into();
    n
}

#[test]
fn textarea_alias_resolves_to_textfield_query() {
    let query = FindQuery::from_args(&FindArgs {
        app: None,
        role: Some("textarea".into()),
        name: None,
        value: None,
        text: None,
        count: false,
        first: false,
        last: false,
        nth: None,
        limit: None,
    });

    assert_eq!(query.role.as_deref(), Some("textfield"));

    let root = node(None, Some("doc body"), None);
    let mut matches = Vec::new();
    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);
    assert_eq!(matches.len(), 1);
}

#[test]
fn unknown_role_passes_through_and_matches_nothing() {
    let query = FindQuery::from_args(&FindArgs {
        app: None,
        role: Some("navbar".into()),
        name: None,
        value: None,
        text: None,
        count: false,
        first: false,
        last: false,
        nth: None,
        limit: None,
    });

    assert_eq!(query.role.as_deref(), Some("navbar"));

    let root = role_node("textfield", Some("body"));
    let mut matches = Vec::new();
    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);
    assert!(matches.is_empty());
}

#[test]
fn empty_role_filtered_result_reports_roles_present_from_tree() {
    let mut root = role_node("window", Some("Save"));
    root.children = vec![
        role_node("button", Some("OK")),
        role_node("textfield", None),
    ];

    let query = FindQuery::from_args(&FindArgs {
        app: None,
        role: Some("navbar".into()),
        name: None,
        value: None,
        text: None,
        count: false,
        first: false,
        last: false,
        nth: None,
        limit: None,
    });
    let response = single_match_response(None, &query, &root);

    let present = response["roles_present"].as_array().unwrap();
    let names: Vec<&str> = present.iter().filter_map(|v| v.as_str()).collect();
    assert!(names.contains(&"button"));
    assert!(names.contains(&"textfield"));
    assert!(names.contains(&"window"));
    assert!(!names.contains(&"navbar"));
}

#[test]
fn roles_present_hint_is_omitted_when_a_match_is_found() {
    let root = role_node("textfield", Some("body"));
    let query = FindQuery::from_args(&FindArgs {
        app: None,
        role: Some("textfield".into()),
        name: None,
        value: None,
        text: None,
        count: false,
        first: false,
        last: false,
        nth: None,
        limit: None,
    });

    let mut matches = Vec::new();
    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);
    let mut response = json!({ "matches": matches });
    let is_empty = response["matches"].as_array().unwrap().is_empty();
    attach_roles_present_hint(&mut response, is_empty, &query, &root);

    assert!(response.get("roles_present").is_none());
}
