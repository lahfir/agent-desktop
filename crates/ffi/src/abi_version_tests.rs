use super::*;

/// `AdRefEntry` and `AdActionStep` are pinned `repr(C)` structs that a
/// prebuilt C consumer reads by fixed offset. Reordering `native_id` inside
/// `AdRefEntry` (F7) is a breaking layout change, so the major must be bumped
/// past the last version a consumer could have compiled against. Pinning the
/// literal here (rather than only comparing `ad_abi_version()` to the Rust
/// constant, which trivially agrees with itself) makes an accidental revert
/// of the bump fail this test even though the two symbols would still match
/// each other.
#[test]
fn abi_major_is_bumped_past_the_ref_entry_layout_break() {
    let major = std::hint::black_box(AD_ABI_VERSION_MAJOR);
    assert!(
        major >= 2,
        "AdRefEntry's native_id field moved to the end of the struct (F7); \
         AD_ABI_VERSION_MAJOR must be bumped to at least 2 so a consumer built \
         against the old layout fails ad_init instead of misreading fields"
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
