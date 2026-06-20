use crate::{
    adapter::PlatformAdapter, context::CommandContext, error::AppError, node::AccessibilityNode,
    roles, search_text, snapshot,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const DEFAULT_LIMIT: usize = 50;

pub struct FindArgs {
    pub app: Option<String>,
    pub role: Option<String>,
    pub name: Option<String>,
    pub value: Option<String>,
    pub text: Option<String>,
    pub count: bool,
    pub first: bool,
    pub last: bool,
    pub nth: Option<usize>,
    pub limit: Option<usize>,
}

pub fn execute(
    args: FindArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    validate_find_mode(&args)?;
    let query = FindQuery::from_args(&args);
    let opts = crate::adapter::TreeOptions::default();
    let result = if args.count {
        snapshot::build(adapter, &opts, args.app.as_deref(), None)?
    } else {
        snapshot::run_with_context(adapter, &opts, args.app.as_deref(), None, context)?
    };

    if args.count {
        return Ok(json!({ "count": count_matches(&result.tree, &query) }));
    }

    let mut matches = Vec::new();
    let max_matches = max_matches_for_args(&args);
    search_tree(
        &result.tree,
        &query,
        &mut Vec::new(),
        &mut matches,
        max_matches,
    );

    if args.first {
        return Ok(single_match_response(
            matches.into_iter().next(),
            &query,
            &result.tree,
        ));
    }

    if args.last {
        return Ok(single_match_response(
            matches.into_iter().last(),
            &query,
            &result.tree,
        ));
    }

    if let Some(n) = args.nth {
        return Ok(single_match_response(
            matches.into_iter().nth(n),
            &query,
            &result.tree,
        ));
    }

    let mut response = json!({ "matches": matches });
    attach_roles_present_hint(&mut response, matches.is_empty(), &query, &result.tree);
    Ok(response)
}

/// When a role-filtered search returns nothing, the caller cannot tell
/// "no elements of this role are on screen" from "this role name does not
/// exist." Listing the roles actually present in the searched tree answers
/// that from live data — no hardcoded vocabulary, so a role any adapter
/// newly emits shows up here automatically.
fn attach_roles_present_hint(
    response: &mut Value,
    is_empty: bool,
    query: &FindQuery,
    tree: &AccessibilityNode,
) {
    if !is_empty || query.role.is_none() {
        return;
    }
    let mut present = BTreeSet::new();
    collect_roles(tree, &mut present);
    if let Some(obj) = response.as_object_mut() {
        obj.insert(
            "roles_present".into(),
            json!(present.into_iter().collect::<Vec<_>>()),
        );
    }
}

fn single_match_response(
    found: Option<Value>,
    query: &FindQuery,
    tree: &AccessibilityNode,
) -> Value {
    let is_empty = found.is_none();
    let mut response = json!({ "match": found });
    attach_roles_present_hint(&mut response, is_empty, query, tree);
    response
}

fn collect_roles(node: &AccessibilityNode, roles: &mut BTreeSet<String>) {
    roles.insert(node.role.clone());
    for child in &node.children {
        collect_roles(child, roles);
    }
}

fn max_matches_for_args(args: &FindArgs) -> Option<usize> {
    if args.count || args.last {
        return None;
    }
    if args.first {
        return Some(1);
    }
    if let Some(n) = args.nth {
        return Some(n.saturating_add(1));
    }
    match args.limit.unwrap_or(DEFAULT_LIMIT) {
        0 => None,
        limit => Some(limit),
    }
}

fn validate_find_mode(args: &FindArgs) -> Result<(), AppError> {
    let selector_count = [args.count, args.first, args.last, args.nth.is_some()]
        .into_iter()
        .filter(|selected| *selected)
        .count();
    if selector_count > 1 || (selector_count == 1 && args.limit.is_some()) {
        return Err(AppError::invalid_input_with_suggestion(
            "find accepts only one result-shaping mode",
            "Use one of --count, --first, --last, --nth, or --limit.",
        ));
    }
    Ok(())
}

#[derive(Debug)]
struct FindQuery {
    role: Option<String>,
    name: Option<String>,
    value: Option<String>,
    text: Option<String>,
}

impl FindQuery {
    fn from_args(args: &FindArgs) -> Self {
        Self {
            role: args.role.as_deref().map(roles::normalize_role_query),
            name: args.name.as_deref().map(search_text::normalize),
            value: args.value.as_deref().map(search_text::normalize),
            text: args.text.as_deref().map(search_text::normalize),
        }
    }
}

fn search_tree(
    node: &AccessibilityNode,
    query: &FindQuery,
    path: &mut Vec<String>,
    matches: &mut Vec<Value>,
    max_matches: Option<usize>,
) -> bool {
    if max_matches.is_some_and(|limit| matches.len() >= limit) {
        return true;
    }
    if node_matches(node, query) {
        let interactive = node.ref_id.is_some();
        let display_name = node
            .name
            .as_deref()
            .or(node.value.as_deref())
            .or(node.description.as_deref())
            .map(String::from)
            .unwrap_or_else(|| format!("(unnamed {})", node.role));
        matches.push(json!({
            "ref_id": node.ref_id,
            "role": node.role,
            "name": display_name,
            "value": node.value,
            "states": node.states,
            "interactive": interactive,
            "path": path.clone()
        }));
        if max_matches.is_some_and(|limit| matches.len() >= limit) {
            return true;
        }
    }

    let label = node
        .name
        .as_deref()
        .or(node.value.as_deref())
        .map(|label| format!("{}:{label}", node.role))
        .unwrap_or_else(|| node.role.clone());
    path.push(label);

    for child in &node.children {
        if search_tree(child, query, path, matches, max_matches) {
            path.pop();
            return true;
        }
    }

    path.pop();
    false
}

fn count_matches(node: &AccessibilityNode, query: &FindQuery) -> usize {
    usize::from(node_matches(node, query))
        + node
            .children
            .iter()
            .map(|child| count_matches(child, query))
            .sum::<usize>()
}

fn node_matches(node: &AccessibilityNode, query: &FindQuery) -> bool {
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

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
