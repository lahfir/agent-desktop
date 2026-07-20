use super::*;

/// ABI 2 shipped the expanded result/ref layouts. Replacing the three-argument
/// `ad_dismiss_notification` with the identity-required five-argument contract
/// breaks callers compiled against ABI 2, so this branch must advertise ABI 3.
#[test]
fn abi_major_covers_the_checked_dismiss_signature() {
    let major = std::hint::black_box(AD_ABI_VERSION_MAJOR);
    assert!(
        major >= 3,
        "identity-required ad_dismiss_notification is incompatible with ABI 2"
    );
}

#[test]
fn ad_init_accepts_current_major_and_rejects_stale_major() {
    assert_eq!(ad_init(AD_ABI_VERSION_MAJOR), AdResult::Ok);
    assert_eq!(ad_init(AD_ABI_VERSION_MAJOR - 1), AdResult::ErrInvalidArgs);
}

#[test]
fn ad_abi_version_reports_the_compiled_major() {
    assert_eq!(ad_abi_version(), AD_ABI_VERSION_MAJOR);
}
