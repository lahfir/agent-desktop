use super::*;
use crate::input::mouse_send::mouse_send_fake_sink as sink;
use agent_desktop_core::{AdapterError, DeliverySemantics};

const LEFT_UP: u32 = 0x0004;

#[test]
fn never_armed_guard_reports_not_delivered_and_no_release() {
    let guard = ClickReleaseGuard::new(LEFT_UP);
    let error = guard.enrich_error(AdapterError::internal("pre-post failure"));

    assert_eq!(error.disposition, DeliverySemantics::not_delivered());
    assert!(!guard.should_release());
}

#[test]
fn armed_guard_with_deliveries_reports_delivered_unverified_and_emergency_release() {
    let mut guard = ClickReleaseGuard::new(LEFT_UP);
    guard.arm();
    for _ in 0..2 {
        guard.mark_delivered();
    }
    let error = guard.enrich_error(AdapterError::timeout("deadline"));

    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let details = error.details.expect("enriched details");
    assert_eq!(details["delivered_events"], 2);
    assert_eq!(details["emergency_release_posted"], true);
    assert_eq!(details["emergency_release_acknowledged"], false);
}

#[test]
fn disarmed_guard_posts_nothing_on_drop() {
    sink::reset();
    {
        let mut guard = ClickReleaseGuard::new(LEFT_UP);
        guard.arm();
        guard.mark_delivered();
        guard.disarm();
    }

    assert!(
        sink::recorded().is_empty(),
        "a normally completed click must not post a corrective release"
    );
}

#[test]
fn armed_guard_posts_corrective_up_on_drop() {
    sink::reset();
    {
        let mut guard = ClickReleaseGuard::new(LEFT_UP);
        guard.arm();
        guard.mark_delivered();
    }

    let recorded = sink::recorded();
    assert_eq!(recorded.len(), 1, "corrective button-up only");
    assert_eq!(recorded[0].flags, LEFT_UP);
}
