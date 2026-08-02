/// A boundary node is read for its child count alone, purely to annotate how
/// much was left unobserved. On a renderer that materialises children lazily
/// that count is not cheap — asking for it can cost more than the traversal it
/// was describing, and a single one could consume an entire snapshot budget.
/// The count is therefore best-effort: it gets a small slice, and a boundary
/// that cannot afford one is still reported as truncated, just without a number.
const BOUNDARY_COUNT_BUDGET: std::time::Duration = std::time::Duration::from_millis(25);

pub(super) fn boundary_count_deadline(
    max_elements: usize,
    deadline: std::time::Instant,
) -> std::time::Instant {
    if max_elements == 0 {
        deadline.min(std::time::Instant::now() + BOUNDARY_COUNT_BUDGET)
    } else {
        deadline
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn a_boundary_count_gets_only_a_slice_of_a_long_deadline() {
        let far = Instant::now() + Duration::from_secs(3);

        let bounded = boundary_count_deadline(0, far);

        assert!(
            bounded < far,
            "a boundary node is read for its count alone; letting one lazy renderer container \
             spend the whole snapshot budget on it is what this bound exists to prevent"
        );
        assert!(bounded <= Instant::now() + Duration::from_millis(60));
    }

    #[test]
    fn a_real_child_read_keeps_the_callers_deadline() {
        let far = Instant::now() + Duration::from_secs(3);

        assert_eq!(boundary_count_deadline(128, far), far);
    }

    #[test]
    fn the_bound_never_extends_a_deadline_that_is_already_shorter() {
        let near = Instant::now() + Duration::from_millis(5);

        assert_eq!(boundary_count_deadline(0, near), near);
    }
}
