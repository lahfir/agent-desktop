use super::imp::{HitClassification, classify_relation};

#[test]
fn self_hit_reaches_target() {
    assert_eq!(
        classify_relation(true, false),
        HitClassification::ReachesTarget
    );
}

#[test]
fn reaches_target_takes_priority_over_ancestor() {
    assert_eq!(
        classify_relation(true, true),
        HitClassification::ReachesTarget
    );
}

#[test]
fn ancestor_hit_is_reported_separately_from_unrelated() {
    assert_eq!(
        classify_relation(false, true),
        HitClassification::AncestorOfTarget
    );
}

#[test]
fn unrelated_hit_is_neither_target_nor_ancestor() {
    assert_eq!(
        classify_relation(false, false),
        HitClassification::Unrelated
    );
}
