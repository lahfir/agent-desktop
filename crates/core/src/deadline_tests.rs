use std::time::Duration;

use crate::{Deadline, ErrorCode};

#[test]
fn oversized_deadline_is_rejected() {
    assert_eq!(
        Deadline::after(u64::MAX).unwrap_err().code,
        ErrorCode::InvalidArgs
    );
}

#[test]
fn slices_never_extend_the_parent_budget() {
    let deadline = Deadline::after(100).unwrap();
    assert!(
        deadline.remaining_slice(Duration::from_secs(1)).unwrap() <= Duration::from_millis(100)
    );
}
