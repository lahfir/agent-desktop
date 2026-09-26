//! The path tier's second reading of a stored path, for the one drift a Win32
//! container causes on its own: growing scroll bars ahead of its content.
//!
//! A `SysTreeView32` or `SysListView32` whose content outgrows the view gains a
//! vertical scroll bar, a horizontal one and the size grip between them, and
//! the proxy exposes all three as the container's *first* children. Every
//! content child's raw index moves up by the chrome count, so a path recorded
//! before the growth now descends into a scroll bar. Regedit measured it: the
//! outline's children went from `[Computer]` to `[scrollbar, scrollbar,
//! handle, Computer]` the moment `HKEY_CLASSES_ROOT` expanded, and the broad
//! search the declined landing fell through to then read every one of the
//! expanded key's thousands of subkeys and spent the whole deadline.

use agent_desktop_core::RefEntry;

use super::resolve_search::{
    NodeAdmission, SEARCH_DESCENT, admit_node, can_use_path_fast_path, descent,
};
use super::walker::{TreeSource, WalkBudget};

const LEADING_CHROME_ROLES: [&str; 2] = ["scrollbar", "handle"];

const MAX_LEADING_CHROME: usize = 3;

const SIBLING_CHECK_CAP: usize = 64;

const MAX_CANDIDATE_PATHS: usize = 4;

/// Re-walks the stored path allowing for leading chrome that appeared after
/// the ref was recorded, and answers only a landing the shift produced that
/// is the single candidate answering to the stored identity.
///
/// Leading chrome is the two scroll bars and the size grip (at most three
/// children, roles `scrollbar` and `handle`). At a level that carries it the
/// stored index is ambiguous: recorded before the chrome appeared it counts
/// content only, recorded after it counts the chrome too. A stored index that
/// lands inside the chrome can only be the former, so it is shifted; any
/// other index is tried both as stored and shifted. Every candidate landing
/// is checked against the stored identity among its own siblings (a list
/// wider than 64 is not read and declines), and the repair answers only when
/// exactly one candidate matches and it was shifted. Two matches - a path
/// that already counted the chrome, whose shift reaches a neighbouring
/// parent with a same-named child - decline, as does a match only on the
/// unshifted path the plain tier already refused. More than four candidate
/// paths, or a read that faults on any of them, also declines, leaving the
/// attempt to the broad search.
///
/// The stored bounds are deliberately not consulted. A container that grew
/// scroll bars also re-laid out and usually scrolled, so the stored rectangle
/// names a position that no longer exists.
pub(crate) fn repair_past_leading_chrome<S: TreeSource>(
    source: &S,
    root: &S::Node,
    entry: &RefEntry,
    budget: &WalkBudget,
) -> Option<S::Node> {
    if !can_use_path_fast_path(entry) {
        return None;
    }
    let mut frontier: Vec<(Option<S::Node>, S::Node, bool)> = vec![(None, root.clone(), false)];
    for &index in entry.scope.path.iter() {
        let mut next = Vec::new();
        for (_, node, shifted) in &frontier {
            let take = index.saturating_add(MAX_LEADING_CHROME + 1);
            let Ok(children) =
                descent::read_children(source, node, budget, &SEARCH_DESCENT, Some(take))
            else {
                return None;
            };
            let chrome = children
                .elements
                .iter()
                .take(MAX_LEADING_CHROME)
                .take_while(|child| is_leading_chrome(source, child))
                .count();
            if index >= chrome {
                if let Some(child) = children.elements.get(index) {
                    next.push((Some(node.clone()), child.clone(), *shifted));
                }
            }
            if chrome > 0 {
                if let Some(child) = children.elements.get(index + chrome) {
                    next.push((Some(node.clone()), child.clone(), true));
                }
            }
        }
        if next.is_empty() || next.len() > MAX_CANDIDATE_PATHS {
            return None;
        }
        frontier = next;
    }
    let mut matched = frontier.into_iter().filter(|(parent, node, _)| {
        parent
            .as_ref()
            .is_some_and(|parent| is_unique_identity_match(source, parent, node, entry, budget))
    });
    let (_, only, shifted) = matched.next()?;
    (matched.next().is_none() && shifted).then_some(only)
}

fn is_leading_chrome<S: TreeSource>(source: &S, node: &S::Node) -> bool {
    let (_, evidence, _) = source.evidence(node);
    evidence
        .role
        .known()
        .is_some_and(|role| LEADING_CHROME_ROLES.contains(&role.as_str()))
}

fn is_unique_identity_match<S: TreeSource>(
    source: &S,
    parent: &S::Node,
    landing: &S::Node,
    entry: &RefEntry,
    budget: &WalkBudget,
) -> bool {
    let Ok(siblings) = descent::read_children(
        source,
        parent,
        budget,
        &SEARCH_DESCENT,
        Some(SIBLING_CHECK_CAP + 1),
    ) else {
        return false;
    };
    if !siblings.complete || siblings.elements.len() > SIBLING_CHECK_CAP {
        return false;
    }
    let mut matched = None;
    for sibling in &siblings.elements {
        let (_, evidence, _) = source.evidence(sibling);
        match admit_node(entry, &evidence) {
            NodeAdmission::Collect if matched.is_none() => matched = Some(sibling),
            NodeAdmission::Collect | NodeAdmission::Unread => return false,
            NodeAdmission::Reject => {}
        }
    }
    matched.is_some_and(|only| source.same_element(only, landing))
}

#[cfg(test)]
#[path = "resolve_chrome_repair_tests.rs"]
mod tests;
