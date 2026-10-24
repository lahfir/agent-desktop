use super::*;
use crate::tree::live::test_support::rect;

fn twins() -> Vec<Option<Rect>> {
    vec![
        Some(rect(391.0, 196.0, 34.0, 34.0)),
        Some(rect(430.0, 196.0, 23.0, 34.0)),
    ]
}

#[test]
fn a_zero_area_stored_size_cannot_separate_twins() {
    let flat = rect(10.0, 10.0, 34.0, 0.0);
    assert_eq!(
        select_by_stored_geometry(&twins(), &[], Some(&flat), BoundsSpace::Screen),
        None
    );
}

#[test]
fn two_matches_with_no_usable_stored_size_are_ambiguous_even_with_an_unread_region() {
    assert_eq!(
        classify(&twins(), &[], None, BoundsSpace::Screen, true),
        Verdict::Ambiguous
    );
}

#[test]
fn a_size_separable_pair_beside_an_unread_region_is_incomplete() {
    let stored = rect(0.0, 0.0, 23.0, 34.0);
    assert_eq!(
        classify(&twins(), &[], Some(&stored), BoundsSpace::Screen, true),
        Verdict::Incomplete
    );
}
