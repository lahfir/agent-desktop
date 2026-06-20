use crate::adapter::SnapshotSurface;
use crate::node::AccessibilityNode;
use crate::refs::{RefEntry, RefMap};

pub(crate) use crate::roles::INTERACTIVE_ROLES;

pub(crate) fn ref_entry_from_node(
    node: &AccessibilityNode,
    pid: i32,
    source_app: Option<&str>,
    source_window_id: Option<&str>,
    source_window_title: Option<&str>,
    root_ref: Option<String>,
    path: &[usize],
) -> RefEntry {
    RefEntry {
        pid,
        role: node.role.clone(),
        name: meaningful_string(node.name.clone()),
        value: meaningful_string(node.value.clone()),
        description: meaningful_string(node.description.clone()),
        states: node.states.clone(),
        bounds: node.bounds,
        bounds_hash: node.bounds.as_ref().map(|b| b.bounds_hash()),
        available_actions: if node.available_actions.is_empty() {
            crate::capability::defaults_for_role(&node.role)
        } else {
            node.available_actions.clone()
        },
        source_app: source_app.map(str::to_string),
        source_window_id: source_window_id.map(str::to_string),
        source_window_title: source_window_title.map(str::to_string),
        source_surface: SnapshotSurface::Window,
        root_ref,
        path_is_absolute: false,
        path: smallvec::SmallVec::from_slice(path),
    }
}

/// An element receives a ref when it is addressable for an action: either its
/// role is interactive, or it advertises an available action regardless of
/// role. Container roles like `scrollarea` (Scroll) and `disclosure`
/// (Expand/Collapse) are not "interactive" by role but are genuinely
/// actionable, and `scroll` / `expand` / `collapse` need a ref to target
/// them — so action-bearing elements must be ref-able. A bare `SetFocus`
/// affordance does not qualify on its own: focusability is not a primary
/// action and would ref-allocate large numbers of inert containers.
pub(crate) fn is_ref_able(node: &AccessibilityNode) -> bool {
    INTERACTIVE_ROLES.contains(&node.role.as_str()) || advertises_primary_action(node)
}

fn advertises_primary_action(node: &AccessibilityNode) -> bool {
    node.available_actions
        .iter()
        .any(|action| action != crate::capability::SET_FOCUS)
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
    pub source_window_id: Option<&'a str>,
    pub source_window_title: Option<&'a str>,
    pub source_surface: SnapshotSurface,
    pub root_ref_id: Option<&'a str>,
    pub path_prefix: &'a [usize],
}

pub(crate) fn allocate_refs(
    node: AccessibilityNode,
    refmap: &mut RefMap,
    config: &RefAllocConfig,
) -> AccessibilityNode {
    allocate_refs_at_path(node, refmap, config, &mut config.path_prefix.to_vec())
}

fn allocate_refs_at_path(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    config: &RefAllocConfig,
    path: &mut Vec<usize>,
) -> AccessibilityNode {
    let is_ref_able = is_ref_able(&node);

    if is_ref_able {
        let mut entry = ref_entry_from_node(
            &node,
            config.pid,
            config.source_app,
            config.source_window_id,
            config.source_window_title,
            config.root_ref_id.map(str::to_string),
            path,
        );
        entry.source_surface = config.source_surface;
        entry.path_is_absolute = config.root_ref_id.is_some();
        strip_ref_bounds_when_hidden(&mut entry, config.include_bounds);
        node.ref_id = Some(refmap.allocate(entry));
    }

    let has_label = node.name.as_deref().is_some_and(|n| !n.is_empty())
        || node.description.as_deref().is_some_and(|d| !d.is_empty());
    let is_skeleton_anchor =
        !is_ref_able && node.children_count.is_some() && has_label && config.root_ref_id.is_none();

    if is_skeleton_anchor {
        let mut entry = ref_entry_from_node(
            &node,
            config.pid,
            config.source_app,
            config.source_window_id,
            config.source_window_title,
            None,
            path,
        );
        entry.source_surface = config.source_surface;
        entry.available_actions = vec![];
        strip_ref_bounds_when_hidden(&mut entry, config.include_bounds);
        node.ref_id = Some(refmap.allocate(entry));
    }

    if !config.include_bounds {
        node.bounds = None;
    }

    node.children = node
        .children
        .into_iter()
        .enumerate()
        .filter_map(|child| {
            let (idx, child) = child;
            path.push(idx);
            let child = allocate_refs_at_path(child, refmap, config, path);
            path.pop();
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

fn strip_ref_bounds_when_hidden(entry: &mut RefEntry, include_bounds: bool) {
    if !include_bounds {
        entry.bounds = None;
    }
}

fn meaningful_string(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

#[cfg(test)]
#[path = "ref_alloc_tests.rs"]
mod tests;
