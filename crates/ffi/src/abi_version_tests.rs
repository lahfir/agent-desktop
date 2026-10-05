use super::*;

#[test]
fn abi_major_covers_the_removal_of_the_legacy_entrypoints() {
    assert_eq!(AD_ABI_VERSION_MAJOR, 5);
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
