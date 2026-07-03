use crate::{
    adapter::PlatformAdapter,
    commands::{helpers::resolve_app_pid, query},
    context::CommandContext,
    error::AppError,
    locator::{LocatorQuery, StatePredicate},
    node::AccessibilityNode,
    roles, search_text, snapshot,
};
use serde_json::{Value, json};
use std::collections::BTreeSet;

const DEFAULT_LIMIT: usize = 50;

pub use query::FindQuery;

pub struct FindArgs {
    pub app: Option<String>,
    pub role: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub native_id: Option<String>,
    pub value: Option<String>,
    pub text: Option<String>,
    pub exact: bool,
    pub states: Vec<StatePredicate>,
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
    let query = locator_query_from_args(&args)?;
    query.validate_states().map_err(AppError::Adapter)?;

    if let Ok(pid) = resolve_app_pid(args.app.as_deref(), adapter) {
        match adapter.resolve_query(&query, None, pid) {
            Ok(handles) => {
                return finish_from_live_handles(&args, &query, adapter, context, handles);
            }
            Err(err) if err.code == crate::error::ErrorCode::PlatformNotSupported => {}
            Err(err) => return Err(AppError::Adapter(err)),
        }
    }

    execute_snapshot_search(&args, &query, adapter, context)
}

pub fn parse_state_flag(raw: &str) -> Result<StatePredicate, AppError> {
    let (token, expected) = match raw.split_once('=') {
        Some((token, value)) => {
            let expected = match value {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => {
                    return Err(AppError::invalid_input_with_suggestion(
                        format!("Invalid state flag value '{value}' in '{raw}'"),
                        "Use TOKEN, TOKEN=true, or TOKEN=false.",
                    ));
                }
            };
            (token, expected)
        }
        None => (raw, None),
    };
    Ok(StatePredicate {
        token: token.to_string(),
        expected,
    })
}

fn locator_query_from_args(args: &FindArgs) -> Result<LocatorQuery, AppError> {
    Ok(LocatorQuery {
        role: args.role.as_deref().map(roles::normalize_role_query),
        name: args.name.as_deref().map(search_text::normalize),
        description: args.description.as_deref().map(search_text::normalize),
        native_id: args.native_id.clone(),
        value: args.value.as_deref().map(search_text::normalize),
        has_text: args.text.as_deref().map(search_text::normalize),
        exact: args.exact,
        states: args.states.clone(),
        ..LocatorQuery::default()
    })
}

fn finish_from_live_handles(
    args: &FindArgs,
    query: &LocatorQuery,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    handles: Vec<crate::native_handle::NativeHandle>,
) -> Result<Value, AppError> {
    if args.count {
        return Ok(json!({ "count": handles.len() }));
    }

    let snapshot_result = snapshot::run_with_context(
        adapter,
        &crate::adapter::TreeOptions::default(),
        args.app.as_deref(),
        None,
        context,
    )?;
    let mut snapshot_matches = Vec::new();
    collect_snapshot_matches(
        &snapshot_result.tree,
        query,
        &mut Vec::new(),
        &mut snapshot_matches,
        None,
    );

    let selected = select_live_indices(args, handles.len());
    let matches: Vec<Value> = selected
        .into_iter()
        .filter_map(|index| materialize_match(snapshot_matches.get(index)))
        .collect();

    if args.first || args.last || args.nth.is_some() {
        return Ok(single_match_response(
            matches.into_iter().next(),
            query,
            &snapshot_result.tree,
        ));
    }

    let match_count = matches.len();
    let mut response = json!({ "matches": matches });
    attach_roles_present_hint(
        &mut response,
        match_count == 0,
        query,
        &snapshot_result.tree,
    );
    Ok(response)
}

fn execute_snapshot_search(
    args: &FindArgs,
    query: &LocatorQuery,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    let opts = crate::adapter::TreeOptions::default();
    let result = if args.count {
        snapshot::build(adapter, &opts, args.app.as_deref(), None)?
    } else {
        snapshot::run_with_context(adapter, &opts, args.app.as_deref(), None, context)?
    };

    if args.count {
        return Ok(json!({ "count": count_matches(&result.tree, query) }));
    }

    let mut matches = Vec::new();
    let max_matches = max_matches_for_args(args);
    collect_snapshot_matches(
        &result.tree,
        query,
        &mut Vec::new(),
        &mut matches,
        max_matches,
    );

    if args.first {
        return Ok(single_match_response(
            matches.into_iter().next(),
            query,
            &result.tree,
        ));
    }

    if args.last {
        return Ok(single_match_response(
            matches.into_iter().last(),
            query,
            &result.tree,
        ));
    }

    if let Some(n) = args.nth {
        return Ok(single_match_response(
            matches.into_iter().nth(n),
            query,
            &result.tree,
        ));
    }

    let match_count = matches.len();
    let mut response = json!({ "matches": matches });
    attach_roles_present_hint(&mut response, match_count == 0, query, &result.tree);
    Ok(response)
}

fn select_live_indices(args: &FindArgs, total: usize) -> Vec<usize> {
    if args.first {
        return vec![0].into_iter().filter(|_| total > 0).collect();
    }
    if args.last {
        return total.checked_sub(1).into_iter().collect();
    }
    if let Some(n) = args.nth {
        return (n < total).then_some(n).into_iter().collect();
    }
    let limit = max_matches_for_args(args).unwrap_or(total);
    (0..total.min(limit)).collect()
}

fn materialize_match(snapshot_match: Option<&Value>) -> Option<Value> {
    snapshot_match.cloned()
}

fn attach_roles_present_hint(
    response: &mut Value,
    is_empty: bool,
    query: &LocatorQuery,
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
    query: &LocatorQuery,
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

fn collect_snapshot_matches(
    node: &AccessibilityNode,
    query: &LocatorQuery,
    path: &mut Vec<String>,
    matches: &mut Vec<Value>,
    max_matches: Option<usize>,
) -> bool {
    if max_matches.is_some_and(|limit| matches.len() >= limit) {
        return true;
    }
    if query::node_matches(node, query) {
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
        if collect_snapshot_matches(child, query, path, matches, max_matches) {
            path.pop();
            return true;
        }
    }

    path.pop();
    false
}

fn count_matches(node: &AccessibilityNode, query: &LocatorQuery) -> usize {
    usize::from(query::node_matches(node, query))
        + node
            .children
            .iter()
            .map(|child| count_matches(child, query))
            .sum::<usize>()
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
