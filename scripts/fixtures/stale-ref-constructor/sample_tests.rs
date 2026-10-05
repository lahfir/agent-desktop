//! A `*_tests.rs` file. Its callers are test code and must not be counted,
//! because the attribute that gates them lives in the parent module and is
//! invisible to a single-file scan.

#[test]
fn a_stale_ref_is_reported() {
    let error = AdapterError::stale_ref("a whole sentence, deliberately");
    assert_eq!(error.code, ErrorCode::StaleRef);
}
