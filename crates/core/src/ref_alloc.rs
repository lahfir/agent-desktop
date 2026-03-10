use crate::node::AccessibilityNode;
use crate::refs::RefEntry;

pub(crate) const INTERACTIVE_ROLES: &[&str] = &[
    "button",
    "textfield",
    "checkbox",
    "link",
    "menuitem",
    "tab",
    "slider",
    "combobox",
    "treeitem",
    "cell",
    "radiobutton",
    "incrementor",
    "menubutton",
    "switch",
    "colorwell",
    "dockitem",
];

pub(crate) fn actions_for_role(role: &str) -> Vec<String> {
    match role {
        "button" | "link" | "menuitem" | "tab" | "radiobutton" => vec!["Click".into()],
        "textfield" | "incrementor" => vec!["Click".into(), "SetValue".into(), "SetFocus".into()],
        "checkbox" => vec!["Click".into(), "Toggle".into()],
        "combobox" => vec!["Click".into(), "Select".into()],
        "treeitem" => vec!["Click".into(), "Expand".into(), "Collapse".into()],
        "slider" => vec!["SetValue".into()],
        "cell" => vec!["Click".into()],
        _ => vec!["Click".into()],
    }
}

pub(crate) fn ref_entry_from_node(
    node: &AccessibilityNode,
    pid: i32,
    source_app: Option<&str>,
    root_ref: Option<String>,
) -> RefEntry {
    RefEntry {
        pid,
        role: node.role.clone(),
        name: node.name.clone(),
        value: node.value.clone(),
        states: node.states.clone(),
        bounds: node.bounds,
        bounds_hash: node.bounds.as_ref().map(|b| b.bounds_hash()),
        available_actions: actions_for_role(&node.role),
        source_app: source_app.map(str::to_string),
        root_ref,
    }
}

pub(crate) fn is_collapsible(node: &AccessibilityNode) -> bool {
    node.ref_id.is_none()
        && node.name.as_deref().is_none_or(str::is_empty)
        && node.value.as_deref().is_none_or(str::is_empty)
        && node.description.as_deref().is_none_or(str::is_empty)
        && node.states.is_empty()
        && node.children.len() == 1
}
