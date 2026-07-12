use super::ObservedTree;
use crate::AdapterError;
use std::collections::HashSet;

pub(crate) fn validated_postorder(
    tree: &ObservedTree,
) -> Result<(Vec<usize>, Vec<Option<usize>>), AdapterError> {
    let mut state = (
        vec![0_u8; tree.nodes.len()],
        vec![None; tree.nodes.len()],
        Vec::with_capacity(tree.nodes.len()),
    );
    for root in &tree.roots {
        visit(*root as usize, None, tree, &mut state)?;
    }
    if state.2.len() != tree.nodes.len() {
        return Err(AdapterError::internal(
            "locator tree contains unreachable nodes",
        ));
    }
    let mut document_orders = HashSet::new();
    for node in &tree.nodes {
        if !document_orders.insert(node.document_order) {
            return Err(AdapterError::internal(
                "locator tree has duplicate document_order values",
            ));
        }
    }
    Ok((state.2, state.1))
}

fn visit(
    index: usize,
    parent: Option<usize>,
    tree: &ObservedTree,
    state: &mut (Vec<u8>, Vec<Option<usize>>, Vec<usize>),
) -> Result<(), AdapterError> {
    let mark = state
        .0
        .get(index)
        .copied()
        .ok_or_else(|| AdapterError::internal("locator tree child index is out of bounds"))?;
    if mark == 1 {
        return Err(AdapterError::internal("locator tree contains a cycle"));
    }
    if mark == 2 {
        return Err(AdapterError::internal(
            "locator tree node has multiple parents",
        ));
    }
    let node = tree
        .nodes
        .get(index)
        .ok_or_else(|| AdapterError::internal("locator tree node index is out of bounds"))?;
    let path_matches_edge = match parent {
        Some(parent) => tree.nodes.get(parent).is_some_and(|parent| {
            node.path.len() == parent.path.len() + 1
                && node.path.starts_with(parent.path.as_slice())
        }),
        None => node.path.is_empty(),
    };
    if !path_matches_edge {
        return Err(AdapterError::internal(
            "locator tree node path does not match its edges",
        ));
    }
    state.0[index] = 1;
    state.1[index] = parent;
    let mut previous_native_index = None;
    for child in node.children.iter().copied() {
        let native_index = tree
            .nodes
            .get(child as usize)
            .and_then(|child| child.path.last().copied())
            .ok_or_else(|| AdapterError::internal("locator tree child path is empty"))?;
        if previous_native_index.is_some_and(|previous| previous >= native_index) {
            return Err(AdapterError::internal(
                "locator tree children are not in native document order",
            ));
        }
        previous_native_index = Some(native_index);
        visit(child as usize, Some(index), tree, state)?;
    }
    state.0[index] = 2;
    state.2.push(index);
    Ok(())
}
