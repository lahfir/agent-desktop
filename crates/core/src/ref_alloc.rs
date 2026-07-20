use crate::AccessibilityNode;
use crate::refs::{RefEntry, RefMap};

pub(crate) use crate::ref_alloc_config::RefAllocConfig;

pub(crate) use crate::roles::INTERACTIVE_ROLES;

pub(crate) fn ref_entry_from_node(
    node: &AccessibilityNode,
    source: &crate::ref_alloc_source::RefAllocSource<'_>,
    root_ref: Option<String>,
    path: &[usize],
) -> RefEntry {
    let bounds = node
        .presentation
        .bounds
        .filter(|bounds| bounds.validate().is_ok());
    RefEntry {
        process: crate::RefProcess {
            pid: source.pid,
            process_instance: source.process_instance.map(str::to_string),
        },
        identity: crate::RefEntryIdentity {
            role: node.role.clone(),
            name: meaningful_string(node.identity.name.clone()),
            value: meaningful_string(node.identity.value.clone()),
            description: meaningful_string(node.identity.description.clone()),
            native_id: node
                .identity
                .native_id
                .clone()
                .filter(|id| !id.value.trim().is_empty()),
        },
        geometry: crate::RefGeometry {
            bounds,
            bounds_hash: bounds.and_then(|bounds| bounds.bounds_hash()),
        },
        capabilities: crate::RefCapabilities {
            states: node.presentation.states.clone(),
            available_actions: node.presentation.available_actions.clone(),
        },
        source: crate::RefSource {
            source_app: source.app.map(str::to_string),
            source_window_id: source.window_id.map(str::to_string),
            source_window_title: source.window_title.map(str::to_string),
            source_window_bounds_hash: source.window_bounds_hash,
            source_surface: source.surface,
        },
        scope: crate::RefScope {
            root_ref,
            path_is_absolute: false,
            path: smallvec::SmallVec::from_slice(path),
        },
    }
}

/// An element receives a ref when it is addressable for an action: either its
/// role is interactive, or it advertises an available action regardless of
/// role. Container roles like `scrollarea` (Scroll) and `disclosure`
/// (Expand/Collapse) are not "interactive" by role but are genuinely
/// actionable, and `scroll` / `expand` / `collapse` need a ref to target
/// them — so action-bearing elements must be ref-able even when their current
/// bounds are zero-sized. Visibility remains a live actionability concern. A
/// bare `SetFocus` affordance does not qualify on its own: focusability is not
/// a primary action and would ref-allocate large numbers of inert containers.
pub(crate) fn is_ref_able(node: &AccessibilityNode) -> bool {
    is_ref_able_role_actions(&node.role, &node.presentation.available_actions)
}

pub(crate) fn is_ref_able_role_actions(role: &str, available_actions: &[String]) -> bool {
    INTERACTIVE_ROLES.contains(&role) || advertises_primary_action(available_actions)
}

fn advertises_primary_action(available_actions: &[String]) -> bool {
    available_actions
        .iter()
        .any(|action| action != crate::capability::SET_FOCUS)
}

pub(crate) fn is_collapsible(node: &AccessibilityNode) -> bool {
    crate::Role::from_token(&node.role).is_transparent_wrapper()
        && node.ref_id.is_none()
        && node
            .identity
            .name
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        && node
            .identity
            .value
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        && node
            .identity
            .description
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        && node.identity.native_id.is_none()
        && node
            .presentation
            .hint
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        && node.presentation.states.is_empty()
        && node.presentation.available_actions.is_empty()
        && node.presentation.bounds.is_none()
        && node.children_count.is_none()
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
    node.children = node
        .children
        .into_iter()
        .filter_map(|child| {
            let collapsible = compact && is_collapsible(&child);
            let child = transform_tree(child, include_bounds, interactive_only, compact);
            if collapsible {
                return child.children.into_iter().next();
            }
            if interactive_only
                && !is_ref_able(&child)
                && child.children.is_empty()
                && child.children_count.is_none()
            {
                None
            } else {
                Some(child)
            }
        })
        .collect();

    if !include_bounds {
        node.presentation.bounds = None;
    }

    node
}

pub(crate) fn allocate_refs(
    node: AccessibilityNode,
    refmap: &mut RefMap,
    config: &RefAllocConfig,
) -> Result<AccessibilityNode, crate::AppError> {
    allocate_refs_at_path(node, refmap, config, &mut config.scope.path_prefix.to_vec())
}

fn allocate_refs_at_path(
    mut node: AccessibilityNode,
    refmap: &mut RefMap,
    config: &RefAllocConfig,
    path: &mut Vec<usize>,
) -> Result<AccessibilityNode, crate::AppError> {
    let node_is_ref_able = is_ref_able(&node);

    if node_is_ref_able {
        let mut entry = ref_entry_from_node(
            &node,
            &config.source,
            config.scope.root_ref_id.map(str::to_string),
            path,
        );
        entry.scope.path_is_absolute = config.scope.root_ref_id.is_some();
        if !config.options.include_bounds {
            entry.geometry.bounds = None;
        }
        node.ref_id = allocate_observed_ref(refmap, entry)?;
    }

    let has_label = node
        .identity
        .name
        .as_deref()
        .is_some_and(|name| !name.is_empty())
        || node
            .identity
            .description
            .as_deref()
            .is_some_and(|description| !description.is_empty());
    let is_skeleton_anchor = !node_is_ref_able
        && node.children_count.is_some()
        && has_label
        && config.scope.root_ref_id.is_none();

    if is_skeleton_anchor {
        let mut entry = ref_entry_from_node(&node, &config.source, None, path);
        entry.capabilities.available_actions = vec![];
        if !config.options.include_bounds {
            entry.geometry.bounds = None;
        }
        node.ref_id = allocate_observed_ref(refmap, entry)?;
    }

    if !config.options.include_bounds {
        node.presentation.bounds = None;
    }

    let mut children = Vec::new();
    for (idx, child) in node.children.into_iter().enumerate() {
        let collapsible = config.options.compact && is_collapsible(&child);
        path.push(idx);
        let child = allocate_refs_at_path(child, refmap, config, path)?;
        path.pop();
        if collapsible {
            if let Some(child) = child.children.into_iter().next() {
                children.push(child);
            }
            continue;
        }
        if config.options.interactive_only
            && child.ref_id.is_none()
            && !is_ref_able(&child)
            && child.children.is_empty()
            && child.children_count.is_none()
        {
            continue;
        }
        children.push(child);
    }
    node.children = children;

    Ok(node)
}

fn allocate_observed_ref(
    refmap: &mut RefMap,
    entry: RefEntry,
) -> Result<Option<String>, crate::AppError> {
    match refmap.try_allocate_observed(entry)? {
        crate::ref_allocation::RefAllocation::Allocated(ref_id) => Ok(Some(ref_id)),
        crate::ref_allocation::RefAllocation::SkippedInvalidRole
        | crate::ref_allocation::RefAllocation::SkippedInvalidEntry => Ok(None),
    }
}

fn meaningful_string(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

#[cfg(test)]
#[path = "ref_alloc_tests.rs"]
mod tests;
