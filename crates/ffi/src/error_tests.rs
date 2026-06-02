use super::*;

fn last_error_message_str() -> Option<String> {
    LAST_ERROR.with(|cell| cell.borrow().as_ref().map(|e| e.message.to_owned_string()))
}

fn last_error_suggestion_str() -> Option<String> {
    LAST_ERROR.with(|cell| {
        cell.borrow().as_ref().and_then(|e| {
            e.suggestion
                .as_ref()
                .map(|s| s.to_string_lossy().into_owned())
        })
    })
}

fn last_error_platform_detail_str() -> Option<String> {
    LAST_ERROR.with(|cell| {
        cell.borrow().as_ref().and_then(|e| {
            e.platform_detail
                .as_ref()
                .map(|s| s.to_string_lossy().into_owned())
        })
    })
}

#[test]
fn test_no_error_initially() {
    clear_last_error();
    assert!(last_error_message_str().is_none());
}

#[test]
fn result_discriminants_preserve_existing_abi_values() {
    assert_eq!(AdResult::ErrInternal as i32, -12);
    assert_eq!(AdResult::ErrSnapshotNotFound as i32, -13);
    assert_eq!(AdResult::ErrPolicyDenied as i32, -14);
    assert_eq!(AdResult::ErrAmbiguousTarget as i32, -15);
}

#[test]
fn test_set_and_get_error() {
    let err = AdapterError::new(ErrorCode::ElementNotFound, "element @e5 gone")
        .with_suggestion("run snapshot");
    set_last_error(&err);
    assert_eq!(last_error_code(), AdResult::ErrElementNotFound);
    assert_eq!(last_error_message_str().unwrap(), "element @e5 gone");
    assert_eq!(last_error_suggestion_str().unwrap(), "run snapshot");
    assert!(last_error_platform_detail_str().is_none());
}

#[test]
fn test_clear_error() {
    let err = AdapterError::internal("oops");
    set_last_error(&err);
    clear_last_error();
    assert!(last_error_message_str().is_none());
}

#[test]
fn test_error_isolation_across_threads() {
    clear_last_error();
    let err = AdapterError::internal("thread1");
    set_last_error(&err);

    let handle = std::thread::spawn(|| last_error_message_str().is_none());
    assert!(handle.join().unwrap(), "other thread should see no error");
    assert_eq!(last_error_message_str().unwrap(), "thread1");
}

#[test]
fn test_interior_nul_falls_back_to_static() {
    let err = AdapterError::new(ErrorCode::Internal, "before\0after");
    set_last_error(&err);
    assert_eq!(
        last_error_message_str().unwrap(),
        "(message contained null byte)"
    );
}
