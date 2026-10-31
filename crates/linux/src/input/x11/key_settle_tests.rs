use std::time::{Duration, Instant};

use super::{SETTLE, remaining};

#[test]
fn a_process_that_sent_no_key_does_not_wait() {
    assert_eq!(remaining(None, Instant::now()), None);
}

#[test]
fn a_key_sent_just_now_waits_the_whole_settle() {
    let sent = Instant::now();

    assert_eq!(remaining(Some(sent), sent), Some(SETTLE));
}

#[test]
fn work_done_after_the_key_counts_towards_the_settle() {
    let sent = Instant::now();
    let now = sent + Duration::from_millis(150);

    assert_eq!(
        remaining(Some(sent), now),
        Some(SETTLE - Duration::from_millis(150))
    );
}

#[test]
fn a_key_older_than_the_settle_does_not_wait() {
    let sent = Instant::now();

    assert_eq!(remaining(Some(sent), sent + SETTLE), None);
    assert_eq!(remaining(Some(sent), sent + SETTLE * 2), None);
}
