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

/// Roles a Win32 container inserts ahead of its content once the content
/// outgrows the view.
const LEADING_CHROME_ROLES: [&str; 2] = ["scrollbar", "handle"];

/// The most leading chrome one container carries: two scroll bars and the
/// size grip.
const MAX_LEADING_CHROME: usize = 3;

/// The widest sibling list the uniqueness check reads. A wider one declines,
/// leaving the verdict to the broad search.
const SIBLING_CHECK_CAP: usize = 64;

/// Re-walks the stored path, counting a level's index from the first child
/// after that level's leading chrome only where the stored index now lands
/// *inside* that chrome, and answers the landing only when some level shifted
/// and the landing is the one sibling that answers to the stored identity.
///
/// A level whose stored index lands on content is walked as stored: a path
/// recorded while the scroll bars already existed counts them in its index,
/// and shifting it again would descend into a different parent whose child
/// could share the stored identity.
///
/// The stored bounds are deliberately not consulted. A container that grew
/// scroll bars also re-laid out and usually scrolled, so the stored rectangle
/// names a position that no longer exists; what the bounds hash guards
/// against on the plain path tier - a duplicate sibling that moved into the
/// stored index - is ruled out here by requiring the identity match to be
/// unique among the landing's siblings. Anything short of that, an unread
/// sibling included, declines and leaves the attempt to the broad search.
pub(crate) fn repair_past_leading_chrome<S: TreeSource>(
    source: &S,
    root: &S::Node,
    entry: &RefEntry,
    budget: &WalkBudget,
) -> Option<S::Node> {
    if !can_use_path_fast_path(entry) {
        return None;
    }
    let mut parent = None;
    let mut current = root.clone();
    let mut shifted = false;
    for &index in entry.scope.path.iter() {
        let take = index.saturating_add(MAX_LEADING_CHROME + 1);
        let children =
            descent::read_children(source, &current, budget, &SEARCH_DESCENT, Some(take)).ok()?;
        let chrome = children
            .elements
            .iter()
            .take(MAX_LEADING_CHROME)
            .take_while(|child| is_leading_chrome(source, child))
            .count();
        let lands_in_chrome = index < chrome;
        shifted |= lands_in_chrome;
        let wanted = if lands_in_chrome {
            index + chrome
        } else {
            index
        };
        let next = children.elements.get(wanted)?.clone();
        parent = Some(std::mem::replace(&mut current, next));
    }
    let parent = parent?;
    (shifted && is_unique_identity_match(source, &parent, &current, entry, budget))
        .then_some(current)
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
