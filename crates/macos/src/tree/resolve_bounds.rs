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

#[cfg(test)]
mod tests {
    use agent_desktop_core::node::Rect;

    use super::rects_overlap;

    fn r(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    #[test]
    fn overlapping_rects_intersect() {
        assert!(rects_overlap(
            &r(0.0, 0.0, 10.0, 10.0),
            &r(5.0, 5.0, 10.0, 10.0)
        ));
    }

    #[test]
    fn touching_edge_rects_are_considered_overlapping() {
        assert!(rects_overlap(
            &r(0.0, 0.0, 10.0, 10.0),
            &r(10.0, 0.0, 10.0, 10.0)
        ));
    }

    #[test]
    fn non_overlapping_rects_do_not_intersect() {
        assert!(!rects_overlap(
            &r(0.0, 0.0, 5.0, 5.0),
            &r(10.0, 10.0, 5.0, 5.0)
        ));
    }

    #[test]
    fn contained_rect_is_always_overlapping() {
        assert!(rects_overlap(
            &r(2.0, 2.0, 3.0, 3.0),
            &r(0.0, 0.0, 10.0, 10.0)
        ));
    }
}
