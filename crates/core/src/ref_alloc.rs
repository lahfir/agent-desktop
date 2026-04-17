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

/// Applies `include_bounds`, `interactive_only`, and `compact` semantics
/// to a raw adapter tree **without** allocating refs. Used by the FFI
/// `ad_get_tree` path, which exposes a raw tree (no CLI/JSON ref pipeline).
///
/// - `include_bounds = false` strips `bounds` from every node.
/// - `compact = true` collapses single-child chains whose own node has
///   no semantic payload (same criterion `allocate_refs` uses).
/// - `interactive_only = true` prunes leaves whose role is not in
///   `INTERACTIVE_ROLES` and that have no children and no
///   `children_count` marker. Unlike the ref-allocating variant, the
///   decision is role-based (no ref_id to check), which matches the FFI
///   contract that refs are never set on raw trees.
pub fn transform_tree(
    mut node: AccessibilityNode,
    include_bounds: bool,
    interactive_only: bool,
    compact: bool,
) -> AccessibilityNode {
    if !include_bounds {
        node.bounds = None;
    }

    node.children = node
        .children
        .into_iter()
        .filter_map(|child| {
            let child = transform_tree(child, include_bounds, interactive_only, compact);
            if compact && is_collapsible(&child) {
                return child.children.into_iter().next();
            }
            if interactive_only
                && !INTERACTIVE_ROLES.contains(&child.role.as_str())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{AccessibilityNode, Rect};

    fn node(role: &str, name: Option<&str>) -> AccessibilityNode {
        AccessibilityNode {
            ref_id: None,
            role: role.into(),
            name: name.map(str::to_string),
            value: None,
            description: None,
            hint: None,
            states: vec![],
            bounds: Some(Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            }),
            children_count: None,
            children: vec![],
        }
    }

    #[test]
    fn transform_tree_include_bounds_false_strips_bounds() {
        let n = node("group", None);
        let out = transform_tree(n, false, false, false);
        assert!(out.bounds.is_none());
    }

    #[test]
    fn transform_tree_include_bounds_true_preserves_bounds() {
        let n = node("group", None);
        let out = transform_tree(n, true, false, false);
        assert!(out.bounds.is_some());
    }

    #[test]
    fn transform_tree_interactive_only_prunes_noninteractive_leaves() {
        let mut root = node("window", Some("w"));
        root.children = vec![node("group", None), node("button", Some("OK"))];
        let out = transform_tree(root, true, true, false);
        assert_eq!(out.children.len(), 1);
        assert_eq!(out.children[0].role, "button");
    }

    #[test]
    fn transform_tree_interactive_only_keeps_named_containers_with_children() {
        let mut labeled = node("group", Some("Toolbar"));
        labeled.children = vec![node("button", Some("Save"))];
        let mut root = node("window", Some("w"));
        root.children = vec![labeled];
        let out = transform_tree(root, true, true, false);
        assert_eq!(out.children.len(), 1);
        assert_eq!(out.children[0].children.len(), 1);
    }

    #[test]
    fn transform_tree_compact_collapses_empty_single_child_chain() {
        let mut outer = node("group", None);
        let mut inner = node("group", None);
        inner.children = vec![node("button", Some("Go"))];
        outer.children = vec![inner];
        let mut root = node("window", Some("w"));
        root.children = vec![outer];
        let out = transform_tree(root, true, false, true);
        assert_eq!(out.children.len(), 1);
        assert_eq!(out.children[0].role, "button");
    }

    #[test]
    fn transform_tree_compact_preserves_labeled_containers() {
        let mut named = node("group", Some("Toolbar"));
        named.children = vec![node("button", Some("Save"))];
        let mut root = node("window", Some("w"));
        root.children = vec![named];
        let out = transform_tree(root, true, false, true);
        assert_eq!(out.children.len(), 1);
        assert_eq!(out.children[0].role, "group");
        assert_eq!(out.children[0].name.as_deref(), Some("Toolbar"));
    }
}
