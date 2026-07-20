use super::*;
use std::time::Duration;

#[test]
fn expired_deadline_fails_before_accessibility_inventory() {
    let deadline = Instant::now() - Duration::from_millis(1);

    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    let error = list_surfaces_for_pid(pid, deadline)
        .expect_err("an expired surface inventory must time out");

    assert_eq!(error.code.as_str(), "TIMEOUT");
}
