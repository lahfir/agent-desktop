use crate::{adapter::PlatformAdapter, error::AppError, node::AccessibilityNode, snapshot};
use serde_json::{json, Value};

pub struct FindArgs {
    pub app: Option<String>,
    pub role: Option<String>,
    pub name: Option<String>,
    pub value: Option<String>,
}

pub fn execute(args: FindArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let opts = crate::adapter::TreeOptions::default();
    let result = snapshot::run(adapter, &opts, args.app.as_deref(), None)?;

    let mut matches = Vec::new();
    search_tree(&result.tree, &args, &mut Vec::new(), &mut matches);

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
        node.name.as_deref().is_some_and(|name| {
            name.to_lowercase().contains(&n.to_lowercase())
        })
    });
    let value_match = args.value.as_deref().is_none_or(|v| {
        node.value.as_deref().is_some_and(|val| {
            val.to_lowercase().contains(&v.to_lowercase())
        })
    });

    if role_match && name_match && value_match {
        if let Some(ref_id) = &node.ref_id {
            let path_str: Vec<String> = path.clone();
            matches.push(json!({
                "ref": ref_id,
                "role": node.role,
                "name": node.name,
                "value": node.value,
                "path": path_str
            }));
        }
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
