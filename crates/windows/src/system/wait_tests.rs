//! Windows-only: drives a real `MenuFixture` through `wait_for_menu` in both
//! directions, proving the poll loop against live process and menu state
//! rather than a hand-rolled predicate stub.
//!
//! Every `MenuFixture` here is a re-exec'd child of this same test binary, so
//! it shares its process image name with every other such fixture in this
//! crate. Each test below holds `test_support::FIXTURE_APP_NAME_LOCK` for its
//! fixture's entire lifetime so it cannot coexist with another lock-holding
//! test's fixture and defeat that test's `list_apps`-by-name resolution.
//!
//! `open_true_returns_ok_once_the_fixtures_menu_opens_shortly_after_the_wait_starts`
//! additionally takes `fixture_window::on_screen_stage` even though it
//! stages nothing itself: `menu_state_tests.rs`'s
//! `menu_is_open_is_isolated_to_the_queried_process` forces cross-thread
//! input attachment (`AttachThreadInput`/`SetForegroundWindow`) elsewhere in
//! this crate, which can stall an unrelated thread's message pump, and this
//! test's fixture dispatching a posted window message within its own budget
//! is exactly what that stalls. Taking the stage lock here is the same
//! "guards screen state, not only screen real estate" reasoning
//! `window_ops_tests.rs`'s focused-filter test documents. Where a test needs
//! both locks, `FIXTURE_APP_NAME_LOCK` is always acquired first and
//! `on_screen_stage` second.
//!
//! `a_menu_that_never_transitions_times_out_with_direction_specific_platform_detail`
//! also takes the stage lock for the same reason `menu_state_tests.rs`'s
//! `classic_source_detects_the_fixtures_context_menu_open_and_closed` and its
//! siblings do: its `never_closes` fixture holds a menu open across an
//! assertion window, and any other window gaining OS foreground during that
//! window sends the menu owner `WM_CANCELMODE`, dismissing it and turning
//! the expected timeout into a false "closed".
#![cfg(target_os = "windows")]

use super::*;

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use crate::system::test_support::FIXTURE_APP_NAME_LOCK;
use crate::tree::fixture::bootstrap;
use crate::tree::fixture_menu::MenuFixture;

/// A generous budget for the fixture's own child process to dispatch a
/// posted window message and for the adapter's poll loop to observe the
/// resulting menu-mode transition. Under heavy concurrent test load this
/// crate's fixtures compete for CPU with dozens of other re-exec'd child
/// processes, so a tight budget here flakes on nothing but scheduling delay,
/// the same reasoning `READY_TIMEOUT` (30s) already applies to fixture
/// startup.
const STATE_TIMEOUT: Duration = Duration::from_secs(15);
const SETTLE: Duration = Duration::from_millis(250);
const OPEN_AFTER: Duration = Duration::from_millis(300);
const RECV_TIMEOUT: Duration = Duration::from_secs(20);
const TRANSITION_DEADLINE_MS: u64 = 15_000;

fn deadline(timeout_ms: u64) -> Deadline {
    Deadline::after(timeout_ms).expect("bounded deadline")
}

fn identity_for(pid: ProcessId) -> ProcessIdentity {
    let token = process_identity::token_for_pid(pid)
        .expect("token read")
        .expect("live token");
    ProcessIdentity::new(pid, token)
}

#[test]
fn open_true_returns_ok_once_the_fixtures_menu_opens_shortly_after_the_wait_starts() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let identity = identity_for(ProcessId::from(fixture.process_id()));

    let (sender, receiver) = channel();
    thread::spawn(move || {
        let result = wait_for_menu(identity, true, deadline(TRANSITION_DEADLINE_MS));
        let _ = sender.send(result);
    });

    thread::sleep(OPEN_AFTER);
    fixture.open_context_menu();
    assert!(fixture.wait_for_menu_state(true, STATE_TIMEOUT));

    let result = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("wait_for_menu completed");

    assert!(
        result.is_ok(),
        "expected Ok once the menu opened, got {result:?}"
    );
    assert!(
        fixture.menu_is_up(),
        "the fixture's own independently-tracked state must confirm the menu is open"
    );

    fixture.dismiss_context_menu();
    assert!(fixture.wait_for_menu_state(false, STATE_TIMEOUT));
}

#[test]
fn open_false_waits_and_returns_ok_once_the_already_open_menu_is_dismissed() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    fixture.open_context_menu();
    assert!(fixture.wait_for_menu_state(true, STATE_TIMEOUT));
    thread::sleep(SETTLE);
    let identity = identity_for(ProcessId::from(fixture.process_id()));

    let (sender, receiver) = channel();
    thread::spawn(move || {
        let result = wait_for_menu(identity, false, deadline(TRANSITION_DEADLINE_MS));
        let _ = sender.send(result);
    });

    thread::sleep(OPEN_AFTER);
    fixture.dismiss_context_menu();

    let result = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("wait_for_menu completed");

    assert!(
        result.is_ok(),
        "expected Ok once the fixture dismissed its menu, got {result:?}"
    );
    assert!(fixture.wait_for_menu_state(false, STATE_TIMEOUT));
}

/// Uses a poll-count assertion rather than an elapsed-time one so the
/// property holds regardless of host speed: the predicate is read exactly
/// once when the requested state already holds, proving the deadline is
/// consulted only after that first read rather than gating it.
#[test]
fn already_in_the_requested_state_returns_immediately_reading_the_predicate_once() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let identity = identity_for(ProcessId::from(fixture.process_id()));
    poll_calls::take();

    let result = wait_for_menu(identity, false, deadline(5_000));

    assert!(
        result.is_ok(),
        "the menu is already closed, so the wait must succeed without polling twice"
    );
    assert_eq!(
        poll_calls::take(),
        1,
        "the predicate must be read exactly once when the state already matches"
    );
}

/// Both directions in one test so a copy-paste that gives both directions
/// the same `platform_detail` text fails the final inequality assertion.
#[test]
fn a_menu_that_never_transitions_times_out_with_direction_specific_platform_detail() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let never_opens = MenuFixture::spawn().expect("the menu fixture starts");
    let open_identity = identity_for(ProcessId::from(never_opens.process_id()));

    let open_error = wait_for_menu(open_identity, true, deadline(400))
        .expect_err("a menu that never opens must time out");

    let never_closes = MenuFixture::spawn().expect("the menu fixture starts");
    never_closes.open_context_menu();
    assert!(never_closes.wait_for_menu_state(true, STATE_TIMEOUT));
    thread::sleep(SETTLE);
    let close_identity = identity_for(ProcessId::from(never_closes.process_id()));

    let close_error = wait_for_menu(close_identity, false, deadline(400))
        .expect_err("a menu that never closes must time out");
    never_closes.dismiss_context_menu();
    assert!(never_closes.wait_for_menu_state(false, STATE_TIMEOUT));

    assert_eq!(open_error.code, ErrorCode::Timeout);
    assert_eq!(close_error.code, ErrorCode::Timeout);
    assert_eq!(
        open_error.platform_detail.as_deref(),
        Some("No menu opened before the deadline")
    );
    assert_eq!(
        close_error.platform_detail.as_deref(),
        Some("Menu did not close before the deadline")
    );
    assert_ne!(
        open_error.platform_detail, close_error.platform_detail,
        "the two timeout directions must carry different platform_detail text"
    );
}

/// The pid-recycling half of the identity discipline - a dead process whose
/// pid is reused by an unrelated live process before the next poll - cannot
/// be staged deterministically on demand, the same limitation
/// `menu_is_open_is_isolated_to_the_queried_process` in `menu_state_tests.rs`
/// documents for its own isolation property. What this test proves
/// deterministically is the half that a real termination does exercise: the
/// process-alive re-check the loop runs before every predicate read catches
/// the death and reports `StaleRef`, never a satisfied wait and never a
/// generic timeout.
#[test]
fn a_target_that_exits_mid_wait_returns_stale_ref_not_a_satisfied_wait_or_a_timeout() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let identity = identity_for(ProcessId::from(fixture.process_id()));

    let (sender, receiver) = channel();
    thread::spawn(move || {
        let result = wait_for_menu(identity, true, deadline(10_000));
        let _ = sender.send(result);
    });

    thread::sleep(OPEN_AFTER);
    drop(fixture);

    let result = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("wait_for_menu completed");
    let error = result.expect_err("a terminated target must not report a satisfied wait");

    assert_eq!(error.code, ErrorCode::StaleRef);
}

/// A loose upper bound derived from the interval, not a timing literal
/// (R12): a spinning loop that ignored `remaining_slice` would blow past it
/// by orders of magnitude inside the same wall-clock budget.
const SPIN_GUARD_MAX_POLLS: usize = 40;

#[test]
fn the_poll_loop_does_not_spin_over_a_fixed_timeout() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let identity = identity_for(ProcessId::from(fixture.process_id()));
    poll_calls::take();

    let result = wait_for_menu(identity, true, deadline(500));

    result.expect_err("a menu that never opens must time out rather than spin forever");
    let polls = poll_calls::take();
    assert!(
        polls <= SPIN_GUARD_MAX_POLLS,
        "poll count {polls} over a 500ms wait suggests the loop is not sleeping between reads"
    );
}

/// A target that dies mid-wait must carry the disposition and recovery an
/// agent acts on. `DeliverySemantics` defaults to unknown/unknown, so an
/// error built without an explicit disposition tells a caller nothing about
/// whether the wait delivered anything or whether retrying is safe - and this
/// failure is completely understood at the point it is constructed. The
/// project's own envelope contract says a consumer may read `recovery` only
/// when the retry disposition is `safe`, which a defaulted `unknown` never is.
#[test]
fn a_target_that_died_mid_wait_reports_a_disposition_and_a_recovery() {
    let error = stale_process_error(&ProcessIdentity::new(ProcessId::from(4321u32), "gone"));

    assert_eq!(error.code, ErrorCode::StaleRef);
    assert_eq!(
        error.disposition.delivery(),
        agent_desktop_core::DeliveryDisposition::NotDelivered,
        "a wait only observes, so a dead target delivered nothing"
    );
    assert!(
        error.suggestion.is_some(),
        "a dead target is a recoverable condition and must say how to recover"
    );
    let suggestion = error.suggestion.unwrap_or_default();
    assert!(
        !suggestion.contains("snapshot") && !suggestion.contains("ref"),
        "the recovery must be process-oriented; this surface has no refs or \
         snapshots to refresh, got {suggestion}"
    );
}
