use super::*;
use crate::LocatorQuery;
use crate::context::CommandContext;
use crate::refs_test_support::HomeGuard;

fn node(name: Option<&str>, value: Option<&str>, description: Option<&str>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: Some("@e1".into()),
        role: "textfield".into(),
        identity: crate::NodeIdentity {
            name: name.map(String::from),
            value: value.map(String::from),
            description: description.map(String::from),
            native_id: None,
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: vec![],
    }
}

fn search_tree(
    root: &AccessibilityNode,
    query: &LocatorQuery,
    path: &mut Vec<String>,
    matches: &mut Vec<serde_json::Value>,
    max_matches: Option<usize>,
) {
    collect_snapshot_matches(root, query, path, matches, max_matches);
}

fn query_from_args(args: &FindArgs) -> LocatorQuery {
    locator_query_from_args(args).unwrap()
}

fn no_filter() -> FindFilterArgs {
    FindFilterArgs {
        role: None,
        name: None,
        description: None,
        native_id: None,
        value: None,
        text: None,
        exact: false,
    }
}

fn no_selection() -> FindSelectionArgs {
    FindSelectionArgs {
        count: false,
        first: false,
        last: false,
        nth: None,
        limit: None,
    }
}

#[test]
fn display_name_prefers_value_before_description() {
    let root = node(None, Some("current value"), Some("help text"));
    let query = LocatorQuery::default();
    let mut matches = Vec::new();

    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);

    assert_eq!(matches[0]["name"], "current value");
}

#[test]
fn search_tree_match_uses_ref_id_contract_and_includes_states() {
    let mut root = node(Some("Save"), None, None);
    root.presentation.states = vec!["enabled".into()];
    let query = LocatorQuery::default();
    let mut matches = Vec::new();

    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);

    assert_eq!(matches[0]["ref_id"], "@e1");
    assert!(matches[0].get("ref").is_none());
    assert_eq!(matches[0]["states"], serde_json::json!(["enabled"]));
}

#[test]
fn search_tree_matches_text_across_fields() {
    let root = node(None, Some("Primary"), Some("Secondary"));
    let query = LocatorQuery {
        has_text: Some(search_text::normalize("secondary")),
        ..LocatorQuery::default()
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
        identity: Default::default(),
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: (0..60)
            .map(|i| node(Some(&format!("Button {i}")), None, None))
            .collect(),
    };
    let query = LocatorQuery {
        has_text: Some(search_text::normalize("button")),
        ..LocatorQuery::default()
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
        window_id: None,
        root: None,
        snapshot: None,
        surface: crate::SnapshotSurface::Window,
        filter: no_filter(),
        states: vec![],
        selection: FindSelectionArgs {
            first: true,
            limit: Some(10),
            ..no_selection()
        },
    })
    .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn count_matches_does_not_build_result_json() {
    let root = AccessibilityNode {
        ref_id: None,
        role: "window".into(),
        identity: Default::default(),
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children: vec![
            node(Some("Save"), None, None),
            node(Some("Cancel"), None, None),
        ],
    };
    let query = LocatorQuery {
        has_text: Some(search_text::normalize("a")),
        ..LocatorQuery::default()
    };

    assert_eq!(count_matches(&root, &query), 3);
}

fn role_node(role: &str, name: Option<&str>) -> AccessibilityNode {
    let mut n = node(name, None, None);
    n.role = role.into();
    n
}

#[test]
fn role_alias_is_preserved_until_live_validation() {
    let query = query_from_args(&FindArgs {
        app: None,
        window_id: None,
        root: None,
        snapshot: None,
        surface: crate::SnapshotSurface::Window,
        filter: FindFilterArgs {
            role: Some("textarea".into()),
            ..no_filter()
        },
        states: vec![],
        selection: no_selection(),
    });

    assert_eq!(query.identity.role.as_deref(), Some("textarea"));

    let root = node(None, Some("doc body"), None);
    let mut matches = Vec::new();
    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);
    assert_eq!(matches.len(), 1);
}

#[test]
fn unknown_role_is_preserved_until_validation() {
    let query = query_from_args(&FindArgs {
        app: None,
        window_id: None,
        root: None,
        snapshot: None,
        surface: crate::SnapshotSurface::Window,
        filter: FindFilterArgs {
            role: Some("navbar".into()),
            ..no_filter()
        },
        states: vec![],
        selection: no_selection(),
    });

    assert_eq!(query.identity.role.as_deref(), Some("navbar"));

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

    let query = query_from_args(&FindArgs {
        app: None,
        window_id: None,
        root: None,
        snapshot: None,
        surface: crate::SnapshotSurface::Window,
        filter: FindFilterArgs {
            role: Some("navbar".into()),
            ..no_filter()
        },
        states: vec![],
        selection: no_selection(),
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
    let query = query_from_args(&FindArgs {
        app: None,
        window_id: None,
        root: None,
        snapshot: None,
        surface: crate::SnapshotSurface::Window,
        filter: FindFilterArgs {
            role: Some("textfield".into()),
            ..no_filter()
        },
        states: vec![],
        selection: no_selection(),
    });

    let mut matches = Vec::new();
    search_tree(&root, &query, &mut Vec::new(), &mut matches, None);
    let mut response = json!({ "matches": matches });
    let is_empty = response["matches"].as_array().unwrap().is_empty();
    attach_roles_present_hint(&mut response, is_empty, &query, &root);

    assert!(response.get("roles_present").is_none());
}

fn find_args_scoped_to_window(window_id: &str) -> FindArgs {
    FindArgs {
        app: None,
        window_id: Some(window_id.into()),
        root: None,
        snapshot: None,
        surface: crate::SnapshotSurface::Window,
        filter: FindFilterArgs {
            name: Some("OnlyInWindowTwo".into()),
            ..no_filter()
        },
        states: vec![],
        selection: no_selection(),
    }
}

/// The names a `find` response matched, and nothing else from the envelope.
///
/// A failure message built from the whole response would carry the snapshot id
/// and the session-derived fields alongside the one thing the assertion is
/// about. Test output is not a safe place for those to travel, and a reader
/// debugging a scope failure wants the names that leaked, not a JSON tree.
fn matched_names(response: &Value) -> Vec<String> {
    response["matches"]
        .as_array()
        .expect("matches must be an array")
        .iter()
        .map(|entry| {
            entry["name"]
                .as_str()
                .map_or_else(|| "<unnamed>".to_owned(), str::to_owned)
        })
        .collect()
}

#[test]
fn find_scopes_matches_to_requested_window_id() {
    let _guard = HomeGuard::new();
    let context = CommandContext::default();
    let adapter = super::test_support::LiveFindAdapter::complete();

    let from_window_two = execute(find_args_scoped_to_window("w-2"), &adapter, &context)
        .expect("find scoped to w-2 should succeed");
    let hits = matched_names(&from_window_two);
    assert_eq!(
        hits,
        vec!["OnlyInWindowTwo".to_owned()],
        "window w-2's tree owns the only match"
    );

    let from_window_one = execute(find_args_scoped_to_window("w-1"), &adapter, &context)
        .expect("find scoped to w-1 should succeed");
    let misses = matched_names(&from_window_one);
    assert!(
        misses.is_empty(),
        "window w-1 must not surface window w-2's element \
         (window_id must not be ignored or swapped with app), leaked: {misses:?}"
    );
}
