//! The property every stored-path landing rests on.
//!
//! `read_children` only ever appends, and each of its exits breaks the loop
//! rather than skipping an entry, so a truncated enumeration is a *prefix* of
//! the real child list. `descend_path` applies that one level at a time, so the
//! pins below are stated at one level and compose down the path by the loop
//! that walks it.

use super::*;
use crate::tree::resolve_search::walk_stored_path;
use agent_desktop_core::Deadline;

use super::tests::StubTree;

fn budget() -> WalkBudget {
    WalkBudget::new(10, Deadline::standard().expect("a standard deadline"))
}

/// The lemma the path tier trusts its landing on, and the reason a landing does
/// not have to wait on the rest of the walk: what a truncation removes is a
/// suffix, so every index below it still names the child a whole walk would
/// name there. A mid-list gap - the one shape that would make a landing a
/// guess - is unconstructible, because the enumeration has no way to skip an
/// entry and carry on.
///
/// The unread region is asserted alongside on the very same walks, so this pin
/// is exactly the case both facts are live at once: the landing is trustworthy
/// *and* the walk is not entitled to a negative verdict.
#[test]
fn an_index_a_truncated_walk_reaches_names_the_child_a_whole_walk_names() {
    let truncated = StubTree::with_children(6).faulting_after(3);
    let whole = StubTree::with_children(6);

    for index in 0..3 {
        let landing = walk_stored_path(&truncated, &0, &[index], &budget())
            .expect("a transport fault never surfaces for the search");
        let control =
            walk_stored_path(&whole, &0, &[index], &budget()).expect("an unfaulting walk answers");

        assert_eq!(
            landing.element, control.element,
            "index {index} sits below the truncation, so the truncated walk must land on the \
             same child the whole walk lands on"
        );
        assert!(
            landing.unread_region,
            "the same walk still left a region unread, and that fact must survive beside a \
             landing it does not weaken"
        );
        assert!(
            !control.unread_region,
            "the control read the whole list, or the assertion above is not about truncation"
        );
    }
}

/// The other half of the same property: the truncation is never crossed. An
/// index the read prefix does not reach lands nowhere rather than sliding onto
/// whatever sibling the enumeration did manage to read, so the fall-through to
/// the broad search - not a wrong element - is what a stored index past a gap
/// produces.
#[test]
fn an_index_past_a_truncation_lands_nowhere_rather_than_on_a_neighbour() {
    let truncated = StubTree::with_children(6).faulting_after(3);
    for index in 3..6 {
        let landing = walk_stored_path(&truncated, &0, &[index], &budget())
            .expect("a transport fault never surfaces for the search");
        assert_eq!(
            landing.element, None,
            "index {index} is past the truncation and must land nowhere"
        );
        assert!(landing.unread_region);
    }

    let control = walk_stored_path(&StubTree::with_children(6), &0, &[5], &budget())
        .expect("an unfaulting walk answers");
    assert_eq!(
        control.element,
        Some(6),
        "the same indices are reachable on a whole walk, so the pin above is not vacuous"
    );
}

/// A tier whose eligibility gate declined to run it read nothing, so it has no
/// region of its own to withhold a verdict for. Reporting one would make every
/// ref that skips the path tier - anything with an empty stored path - retry
/// against a gap that no attempt can ever close.
#[test]
fn a_path_tier_that_never_walked_lands_nowhere_and_claims_no_unread_region() {
    let landing = PathLanding::<usize>::not_walked();

    assert!(landing.element.is_none());
    assert!(
        !landing.unread_region,
        "a walk that never ran left nothing unread; the tiers below it still cover the tree"
    );
}
