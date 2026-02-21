use crate::{
    action::Action,
    adapter::{PlatformAdapter, TreeOptions},
    commands::helpers::resolve_ref,
    error::AppError,
    node::AccessibilityNode,
    snapshot,
};
use serde_json::Value;

pub struct RightClickArgs {
    pub ref_id: String,
}

pub fn execute(args: RightClickArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, Action::RightClick)?;
    let mut response = serde_json::to_value(&result)?;

    std::thread::sleep(std::time::Duration::from_millis(200));

    let opts = TreeOptions {
        interactive_only: true,
        ..Default::default()
    };
    if let Ok(snap) = snapshot::build(adapter, &opts, entry.source_app.as_deref(), None) {
        snap.refmap.save().ok();
        if let Some(menu) = find_context_menu(&snap.tree) {
            if let Ok(menu_json) = serde_json::to_value(menu) {
                response["menu"] = menu_json;
            }
        }
    }

    Ok(response)
}

fn find_context_menu(node: &AccessibilityNode) -> Option<&AccessibilityNode> {
    if node.role == "menu" && node.children.iter().any(|c| c.role == "menuitem") {
        return Some(node);
    }
    for child in &node.children {
        if let Some(menu) = find_context_menu(child) {
            return Some(menu);
        }
    }
    None
}
