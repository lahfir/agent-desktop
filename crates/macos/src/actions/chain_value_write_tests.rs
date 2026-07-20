use super::finite_target;

#[test]
fn finite_target_rejects_non_finite_numbers() {
    assert_eq!(finite_target("42.5"), Some(42.5));
    assert_eq!(finite_target("NaN"), None);
    assert_eq!(finite_target("inf"), None);
    assert_eq!(finite_target("-inf"), None);
    assert_eq!(finite_target("not-a-number"), None);
}

#[test]
fn verification_failure_after_first_write_is_unsafe_to_retry() {
    let error = super::verification_failure_after_write(agent_desktop_core::AdapterError::timeout(
        "verification fixture",
    ));

    assert_eq!(
        error.disposition,
        agent_desktop_core::DeliverySemantics::delivered_unverified()
    );
}
