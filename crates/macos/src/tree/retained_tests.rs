use super::*;

fn app(pid: i32) -> AXElement {
    AXElement(unsafe { accessibility_sys::AXUIElementCreateApplication(pid) })
}

#[test]
fn retention_uses_native_equality_and_expires_with_its_owner() {
    let pid = i32::try_from(std::process::id()).unwrap();
    let element = app(pid);
    assert!(capture(&element).unwrap().is_none());
    let owner = RetainedRefSession::start().unwrap();
    assert!(RetainedRefSession::start().is_err());
    let first = capture(&element).unwrap().unwrap();
    let same = capture(&app(pid)).unwrap().unwrap();
    assert_eq!(first, same);
    assert!(matches(&first, &app(pid)).unwrap());
    assert!(!matches(&first, &app(1)).unwrap());
    assert!(capture(&AXElement(std::ptr::null_mut())).is_err());
    assert_eq!(
        evidence(&element, IdentifierEvidence::absent()).retained_object(),
        Some(first.as_str())
    );
    drop(owner);
    assert_eq!(
        validate(Some(&first)).unwrap_err().code,
        agent_desktop_core::ErrorCode::StaleRef
    );
    let _replacement_owner = RetainedRefSession::start().unwrap();
    let replacement = capture(&element).unwrap().unwrap();
    assert_ne!(replacement, first);
    assert!(validate(Some(&first)).is_err());
}

#[test]
fn retention_cannot_cross_threads() {
    let _owner = RetainedRefSession::start().unwrap();
    let token = capture(&app(i32::try_from(std::process::id()).unwrap()))
        .unwrap()
        .unwrap();
    std::thread::spawn(move || {
        assert_eq!(
            validate(Some(&token)).unwrap_err().code,
            agent_desktop_core::ErrorCode::StaleRef
        );
    })
    .join()
    .unwrap();
}
