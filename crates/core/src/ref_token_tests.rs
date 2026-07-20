use crate::ref_token::{qualify_ref_id, resolve_ref_target, validate_ref_token};

#[test]
fn qualified_ref_selects_its_own_snapshot() {
    assert_eq!(
        resolve_ref_target("@sabc:e7", None).unwrap(),
        ("sabc".into(), "@e7".into())
    );
    assert_eq!(qualify_ref_id("sabc", "@e7"), "@sabc:e7");
}

#[test]
fn bare_ref_requires_explicit_snapshot() {
    assert_eq!(
        resolve_ref_target("@e1", None).unwrap_err().code(),
        "INVALID_ARGS"
    );
    assert_eq!(
        resolve_ref_target("@e1", Some("sabc")).unwrap(),
        ("sabc".into(), "@e1".into())
    );
}

#[test]
fn explicit_snapshot_must_match_qualified_ref() {
    assert_eq!(
        resolve_ref_target("@sone:e1", Some("stwo"))
            .unwrap_err()
            .code(),
        "INVALID_ARGS"
    );
}

#[test]
fn syntax_validation_accepts_both_forms() {
    assert!(validate_ref_token("@e1").is_ok());
    assert!(validate_ref_token("@sabc:e1").is_ok());
    assert!(validate_ref_token("@latest:e0").is_err());
}
