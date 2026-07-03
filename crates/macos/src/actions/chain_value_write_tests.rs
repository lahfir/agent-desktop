use super::finite_target;

#[test]
fn finite_target_rejects_non_finite_numbers() {
    assert_eq!(finite_target("42.5"), Some(42.5));
    assert_eq!(finite_target("NaN"), None);
    assert_eq!(finite_target("inf"), None);
    assert_eq!(finite_target("-inf"), None);
    assert_eq!(finite_target("not-a-number"), None);
}
