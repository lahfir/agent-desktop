//! Hit-element classification against a target's ancestor chain.
//!
//! Self or descendant → `ReachesTarget`. Ancestor of the target → `Unknown`
//! directly: unlike macOS, UIA's `ElementFromPoint` is desktop-global with no
//! application-scoped retry, so the arm that would only refine toward
//! `Unknown` is the answer — Chromium render-host panes land here when web
//! content is not hit-addressable. Incomplete ancestry walks → `Unknown`.
//! Unrelated hits hand off to corroboration.

use crate::tree::walker::{DEFAULT_MAX_RAW_DEPTH, NodeKey};
use agent_desktop_core::{Point, Rect};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ancestry {
    Found,
    Absent,
    Incomplete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HitClassification {
    ReachesTarget,
    AncestorOfTarget,
    Unrelated,
}

pub(crate) fn classify_relation(
    reaches_target: bool,
    is_ancestor_of_target: bool,
) -> HitClassification {
    if reaches_target {
        HitClassification::ReachesTarget
    } else if is_ancestor_of_target {
        HitClassification::AncestorOfTarget
    } else {
        HitClassification::Unrelated
    }
}

pub(crate) fn point_in_rect(point: &Point, bounds: &Rect) -> bool {
    point.x >= bounds.x
        && point.y >= bounds.y
        && point.x <= bounds.x + bounds.width
        && point.y <= bounds.y + bounds.height
}

pub(crate) fn intersect_rects(left: Rect, right: Rect) -> Option<Rect> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = (left.x + left.width).min(right.x + right.width);
    let bottom_edge = (left.y + left.height).min(right.y + right.height);
    (right_edge > x && bottom_edge > y).then_some(Rect {
        x,
        y,
        width: right_edge - x,
        height: bottom_edge - y,
    })
}

/// Same-root demotion for unclipped provider rects (A18-2): a candidate point
/// outside `target ∩ nearest-scroll-viewport` must not become `InterceptedBy`.
pub(crate) fn should_demote_outside_viewport(
    point: &Point,
    target_bounds: &Rect,
    viewport: Option<&Rect>,
) -> bool {
    let Some(viewport) = viewport else {
        return false;
    };
    match intersect_rects(*target_bounds, *viewport) {
        Some(visible) => !point_in_rect(point, &visible),
        None => true,
    }
}

pub(crate) fn ancestry_limit() -> usize {
    DEFAULT_MAX_RAW_DEPTH as usize
}

pub(crate) fn remember_ancestor_key<N: Clone>(
    visited_keys: &mut Vec<NodeKey>,
    visited_unkeyed: &mut Vec<N>,
    key: NodeKey,
    current: &N,
    same_element: &dyn Fn(&N, &N) -> bool,
) -> bool {
    match &key {
        NodeKey::Runtime(_) => {
            if visited_keys.iter().any(|seen| seen == &key) {
                return false;
            }
            visited_keys.push(key);
            true
        }
        NodeKey::Unavailable => {
            if visited_unkeyed
                .iter()
                .any(|seen| same_element(seen, current))
            {
                return false;
            }
            visited_unkeyed.push(current.clone());
            true
        }
    }
}

pub(crate) fn ancestry_with<N: Clone>(
    start: &N,
    expected: &N,
    limit: usize,
    same_element: &dyn Fn(&N, &N) -> bool,
    identity: &dyn Fn(&N) -> NodeKey,
    parent_of: &dyn Fn(&N) -> Result<Option<N>, ()>,
) -> Ancestry {
    let mut current = start.clone();
    let mut visited_keys = Vec::new();
    let mut visited_unkeyed = Vec::new();
    for _ in 0..limit {
        if !remember_ancestor_key(
            &mut visited_keys,
            &mut visited_unkeyed,
            identity(&current),
            &current,
            same_element,
        ) {
            return Ancestry::Incomplete;
        }
        let parent = match parent_of(&current) {
            Ok(Some(parent)) => parent,
            Ok(None) => return Ancestry::Absent,
            Err(()) => return Ancestry::Incomplete,
        };
        if same_element(&parent, expected) {
            return Ancestry::Found;
        }
        current = parent;
    }
    Ancestry::Incomplete
}

pub(crate) fn classify_hit_with<N: Clone>(
    target: &N,
    hit: &N,
    same_element: &dyn Fn(&N, &N) -> bool,
    identity: &dyn Fn(&N) -> NodeKey,
    parent_of: &dyn Fn(&N) -> Result<Option<N>, ()>,
) -> Option<HitClassification> {
    let limit = ancestry_limit();
    let reaches_target = if same_element(target, hit) {
        Ancestry::Found
    } else {
        ancestry_with(hit, target, limit, same_element, identity, parent_of)
    };
    if reaches_target == Ancestry::Incomplete {
        return None;
    }
    let is_ancestor_of_target = if reaches_target == Ancestry::Found {
        Ancestry::Absent
    } else {
        ancestry_with(target, hit, limit, same_element, identity, parent_of)
    };
    if is_ancestor_of_target == Ancestry::Incomplete {
        return None;
    }
    Some(classify_relation(
        reaches_target == Ancestry::Found,
        is_ancestor_of_target == Ancestry::Found,
    ))
}
