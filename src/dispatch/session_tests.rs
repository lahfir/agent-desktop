use super::resolve_end_session_id;

#[test]
fn explicit_session_end_id_precedes_active_scope() {
    assert_eq!(
        resolve_end_session_id(Some("explicit".into()), Some("active")).unwrap(),
        "explicit"
    );
}

#[test]
fn session_end_falls_back_to_active_scope() {
    assert_eq!(
        resolve_end_session_id(None, Some("active")).unwrap(),
        "active"
    );
}

#[test]
fn session_end_without_any_scope_is_invalid() {
    let error = resolve_end_session_id(None, None).expect_err("missing scope must fail");

    assert_eq!(error.code(), "INVALID_ARGS");
    assert!(error.to_string().contains("No session id"));
}
