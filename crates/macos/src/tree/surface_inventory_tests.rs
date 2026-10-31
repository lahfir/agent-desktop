use super::*;
use std::time::Duration;

#[test]
fn expired_deadline_fails_before_accessibility_inventory() {
    let deadline = Instant::now() - Duration::from_millis(1);
    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)
        .unwrap()
        .unwrap();
    let error =
        list_surfaces_for_process(identity, deadline).expect_err("expired surface inventory");
    assert_eq!(error.code.as_str(), "TIMEOUT");
}
