use crate::node::AccessibilityNode;
use crate::refs::{RefEntry, RefMap};

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

pub(crate) struct RefAllocConfig<'a> {
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub pid: i32,
    pub source_app: Option<&'a str>,
    pub root_ref_id: Option<&'a str>,
}

pub(crate) fn allocate_refs(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    config: &RefAllocConfig,
) -> AccessibilityNode {
    let root_ref_owned = config.root_ref_id.map(str::to_string);
    let is_interactive = INTERACTIVE_ROLES.contains(&node.role.as_str());

    if is_interactive {
        let entry =
            ref_entry_from_node(&node, config.pid, config.source_app, root_ref_owned.clone());
        node.ref_id = Some(refmap.allocate(entry));
    }

    let has_label = node.name.as_deref().is_some_and(|n| !n.is_empty())
        || node.description.as_deref().is_some_and(|d| !d.is_empty());
    let is_skeleton_anchor = !is_interactive
        && node.children_count.is_some()
        && has_label
        && config.root_ref_id.is_none();

    if is_skeleton_anchor {
        let mut entry = ref_entry_from_node(&node, config.pid, config.source_app, None);
        entry.available_actions = vec![];
        node.ref_id = Some(refmap.allocate(entry));
    }

    if !config.include_bounds {
        node.bounds = None;
    }

    node.children = node
        .children
        .into_iter()
        .filter_map(|child| {
            let child = allocate_refs(child, refmap, config);
            if config.compact && is_collapsible(&child) {
                return child.children.into_iter().next();
            }
            if config.interactive_only
                && child.ref_id.is_none()
                && child.children.is_empty()
                && child.children_count.is_none()
            {
                None
            } else {
                Some(child)
            }
        })
        .collect();

    node
}
