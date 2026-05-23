use agent_desktop_core::{node::Rect, refs::RefEntry};

use super::AXElement;

pub(super) fn bounds_match(el: &AXElement, entry: &RefEntry) -> bool {
    match entry.bounds_hash {
        Some(expected) => {
            let actual = crate::tree::read_bounds(el).map(|b| b.bounds_hash());
            actual.map(|h| h == expected).unwrap_or(false)
        }
        None => true,
    }
}

pub(super) fn should_prune_by_bounds(el: &AXElement, entry: &RefEntry, depth: u8) -> bool {
    if depth == 0 || entry.bounds.is_none() || entry.bounds_hash.is_none() {
        return false;
    }
    let Some(candidate) = crate::tree::read_bounds(el) else {
        return false;
    };
    let Some(target) = entry.bounds.as_ref() else {
        return false;
    };
    !rects_overlap(&candidate, target)
}

fn rects_overlap(candidate: &Rect, target: &Rect) -> bool {
    let candidate_right = candidate.x + candidate.width;
    let candidate_bottom = candidate.y + candidate.height;
    let target_right = target.x + target.width;
    let target_bottom = target.y + target.height;
    candidate.x <= target_right
        && candidate_right >= target.x
        && candidate.y <= target_bottom
        && candidate_bottom >= target.y
}
