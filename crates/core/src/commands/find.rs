use crate::{adapter::PlatformAdapter, error::AppError, node::AccessibilityNode, snapshot};
use serde_json::{json, Value};

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
}

pub fn execute(args: FindArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let opts = crate::adapter::TreeOptions::default();
    let result = snapshot::run(adapter, &opts, args.app.as_deref(), None)?;

    let mut matches = Vec::new();
    search_tree(&result.tree, &args, &mut Vec::new(), &mut matches);

    if args.count {
        return Ok(json!({ "count": matches.len() }));
    }

    if args.first {
        return Ok(json!({ "match": matches.into_iter().next() }));
    }

    if args.last {
        return Ok(json!({ "match": matches.into_iter().last() }));
    }

    if let Some(n) = args.nth {
        return Ok(json!({ "match": matches.into_iter().nth(n) }));
    }

    Ok(json!({ "matches": matches }))
}

fn search_tree(
    node: &AccessibilityNode,
    args: &FindArgs,
    path: &mut Vec<String>,
    matches: &mut Vec<Value>,
) {
    let role_match = args.role.as_deref().is_none_or(|r| node.role == r);
    let name_match = args.name.as_deref().is_none_or(|n| {
        node.name
            .as_deref()
            .is_some_and(|name| name.to_lowercase().contains(&n.to_lowercase()))
    });
    let value_match = args.value.as_deref().is_none_or(|v| {
        node.value
            .as_deref()
            .is_some_and(|val| val.to_lowercase().contains(&v.to_lowercase()))
    });
    let text_match = args.text.as_deref().is_none_or(|t| {
        let t_lower = t.to_lowercase();
        let in_name = node
            .name
            .as_deref()
            .is_some_and(|n| n.to_lowercase().contains(&t_lower));
        let in_value = node
            .value
            .as_deref()
            .is_some_and(|v| v.to_lowercase().contains(&t_lower));
        let in_desc = node
            .description
            .as_deref()
            .is_some_and(|d| d.to_lowercase().contains(&t_lower));
        in_name || in_value || in_desc
    });

    if role_match && name_match && value_match && text_match {
        let interactive = node.ref_id.is_some();
        let display_name = node
            .name
            .as_deref()
            .or(node.description.as_deref())
            .map(String::from)
            .unwrap_or_else(|| format!("(unnamed {})", node.role));
        matches.push(json!({
            "ref": node.ref_id,
            "role": node.role,
            "name": display_name,
            "value": node.value,
            "interactive": interactive,
            "path": path.clone()
        }));
    }

    let label = if let Some(name) = &node.name {
        format!("{}:{}", node.role, name)
    } else {
        node.role.clone()
    };
    path.push(label);

    for child in &node.children {
        search_tree(child, args, path, matches);
    }

    path.pop();
}
