use super::{LocatorMatchData, LocatorSelection, ObservedTree};

pub(crate) fn selected_indices(
    matches: &[usize],
    selection: LocatorSelection,
) -> (Vec<usize>, bool) {
    match selection {
        LocatorSelection::Strict => take(matches, 10),
        LocatorSelection::All {
            limit: Some(0) | None,
        } => (matches.to_vec(), false),
        LocatorSelection::All { limit: Some(limit) } => take(matches, limit as usize),
        LocatorSelection::Count => (Vec::new(), false),
        LocatorSelection::First => take(matches, 1),
        LocatorSelection::Last => (
            matches.last().copied().into_iter().collect(),
            matches.len() > 1,
        ),
        LocatorSelection::Nth(index) => (
            matches.get(index as usize).copied().into_iter().collect(),
            matches.len() > 1,
        ),
    }
}

pub(crate) fn match_data(
    tree: &ObservedTree,
    index: usize,
    parents: &[Option<usize>],
) -> Option<LocatorMatchData> {
    let node = tree.nodes.get(index)?;
    let role = node
        .evidence
        .role
        .known()
        .cloned()
        .unwrap_or_else(|| "unknown".into());
    let name = node
        .evidence
        .name
        .meaningful_string()
        .or_else(|| node.evidence.value.meaningful_string())
        .or_else(|| node.evidence.description.meaningful_string())
        .unwrap_or_else(|| format!("(unnamed {role})"));
    let mut ancestor_indices = Vec::new();
    let mut parent = parents.get(index).copied().flatten();
    while let Some(parent_index) = parent {
        ancestor_indices.push(parent_index);
        parent = parents.get(parent_index).copied().flatten();
    }
    ancestor_indices.reverse();
    let path = ancestor_indices
        .into_iter()
        .filter_map(|ancestor| tree.nodes.get(ancestor))
        .map(node_label)
        .collect();
    Some(LocatorMatchData {
        ref_id: node.ref_id.clone(),
        role,
        name,
        value: node.evidence.value.meaningful_string(),
        states: node.evidence.states.known().cloned().unwrap_or_default(),
        interactive: super::materialize::addressability(&node.evidence).0,
        path,
    })
}

fn take(matches: &[usize], limit: usize) -> (Vec<usize>, bool) {
    (
        matches.iter().take(limit).copied().collect(),
        matches.len() > limit,
    )
}

fn node_label(node: &super::ObservedNode) -> String {
    let role = node
        .evidence
        .role
        .known()
        .map(String::as_str)
        .unwrap_or("unknown");
    node.evidence
        .name
        .meaningful_string()
        .or_else(|| node.evidence.value.meaningful_string())
        .map(|label| format!("{role}:{label}"))
        .unwrap_or_else(|| role.to_string())
}
