//! Live proof that `wait --notification` works on Windows through the
//! existing core loop.
//!
//! The loop under test lives in `agent_desktop_core::commands::wait`: it
//! polls the adapter's listing and diffs a fingerprint multiset against a
//! baseline captured on its own first poll. These tests stage the world that
//! loop observes and drive it through the same `execute` entry the CLI calls,
//! so every scenario runs against the real Action Center and the real
//! Windows listing path - no scenario sleeps past a poll it could have
//! observed instead.
//!
//! The arrival scenario holds the center open for the whole wait because the
//! measured staging behaviour on this host is that a toast joins the center
//! only while the center is open and any close evicts the entry (A26-3). The
//! wait's per-poll sessions adopt an already-present center without closing
//! it, which is exactly what lets a staged arrival survive the polls; a
//! center the wait raised itself would be closed between polls and would
//! evict the entry the wait is for.
//!
//! Every leg holds `FIXTURE_APP_NAME_LOCK` before `SHELL_SURFACE_LOCK`, the
//! crate-wide ordering: the legs raise shell chrome and read the settled
//! foreground, so the two families whose holders stage the desktop under a
//! live test must serialize against this one too.

use std::time::{Duration, Instant};

use agent_desktop_core::{
    AppError, CommandContext, Deadline, ErrorCode, InteractionPolicy, SnapshotSurface,
    commands::wait, commands::wait_surface::SurfaceWait,
};

use super::session::ActionCenterSession;
use crate::adapter::WindowsAdapter;
use crate::notifications::toast_support::{
    self, CloseCenterOnDrop, StagedToast, TOAST_BODY_SECOND, TOAST_TITLE_SECOND,
};
use crate::system::raise_oracle::{responded_since, witness_desktop};
use crate::system::shell_surface::resolve_surface;
use crate::system::shell_surface_open::close_surface;
use crate::system::test_support::{
    FIXTURE_APP_NAME_LOCK, SHELL_SURFACE_LOCK, or_skip_shell, wait_for_foreground_to_settle,
    with_interaction_lease_test_lock,
};

fn deadline(ms: u64) -> Deadline {
    Deadline::after(ms).expect("deadline")
}

fn headed() -> InteractionPolicy {
    InteractionPolicy::headed()
}

fn center_open() -> bool {
    resolve_surface(SnapshotSurface::ActionCenter, deadline(10_000))
        .expect("the desktop is readable")
        .is_some()
}

fn notification_wait_args(timeout_ms: u64) -> wait::WaitArgs {
    wait::WaitArgs {
        mode: wait::WaitModeArgs {
            surface: Some(SurfaceWait::Notification),
            ..Default::default()
        },
        predicate: wait::WaitPredicateArgs {
            snapshot_id: None,
            predicate: None,
            value: None,
            action: None,
            count: None,
        },
        timeout_ms,
        app: None,
    }
}

/// Resets the center, opens it, and holds it open so the staged baseline
/// entry and the staged arrival both land in a center the wait's sessions
/// adopt instead of close. A desktop whose shell declines the open, or whose
/// staging never produces an entry, skips loudly instead: the helper prints
/// why and returns None.
fn hold_center_with_baseline_toast() -> Option<(ActionCenterSession, StagedToast)> {
    toast_support::clear_center(deadline(20_000));
    let witness = witness_desktop();
    let held = or_skip_shell(
        "action center open for the wait baseline",
        ActionCenterSession::open(headed(), deadline(15_000)),
        || responded_since(&witness),
    )?;
    let staged = StagedToast::stage();
    let Some(listed) = toast_support::wait_until_listed_held(held.hwnd(), deadline(30_000)) else {
        eprintln!("skip toast staging: this desktop's toast staging produced no entry");
        return None;
    };
    assert_eq!(
        listed.len(),
        1,
        "the reset leaves the staged toast as the only entry the baseline can capture"
    );
    Some((held, staged))
}

#[test]
fn a_wait_returns_the_notification_that_arrives_during_it_and_not_one_already_present() {
    crate::tree::fixture::bootstrap();
    let _fixture_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some((held, _already_present)) = hold_center_with_baseline_toast() else {
        return;
    };

    let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
    let stager = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(2_000));
        let _staged = StagedToast::stage_with(TOAST_TITLE_SECOND, TOAST_BODY_SECOND);
        let _ = release_rx.recv();
    });

    let started = Instant::now();
    let matched = with_interaction_lease_test_lock(|| {
        wait::execute(
            notification_wait_args(20_000),
            &WindowsAdapter::new(),
            &CommandContext::default().with_headed(true),
        )
        .expect("the wait returns when the staged toast arrives")
    });
    let elapsed = started.elapsed();

    assert_eq!(matched["condition"], "notification");
    assert_eq!(matched["matched"], true);
    assert_eq!(
        matched["notification"]["title"], TOAST_TITLE_SECOND,
        "the wait reports the arrival, never an entry the baseline already carried"
    );
    assert_eq!(matched["notification"]["body"], TOAST_BODY_SECOND);
    assert!(
        elapsed >= Duration::from_millis(2_000),
        "the match is the arrival during the wait, not the baseline: took {elapsed:?}"
    );

    let _ = release_tx.send(());
    stager.join().expect("the staging thread finishes");
    held.close().expect("the held session restores the surface");
}

#[test]
fn a_wait_that_times_out_reports_timeout_and_leaves_the_center_closed() {
    crate::tree::fixture::bootstrap();
    let _fixture_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _cleanup = CloseCenterOnDrop;
    let _ = close_surface(SnapshotSurface::ActionCenter, deadline(8_000));
    assert!(
        wait_for_foreground_to_settle(),
        "the desktop's foreground must settle before the wait is staged"
    );

    let error = with_interaction_lease_test_lock(|| {
        wait::execute(
            notification_wait_args(4_000),
            &WindowsAdapter::new(),
            &CommandContext::default().with_headed(true),
        )
    })
    .expect_err("no notification arrives during the wait");
    let AppError::Adapter(error) = error else {
        panic!("the wait reports a structured adapter error");
    };

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|d| d.get("kind"))
            .and_then(|k| k.as_str()),
        Some("wait_timeout"),
        "the wait loop's own timeout is distinguishable from a chain-deadline timeout"
    );
    assert!(
        !center_open(),
        "each poll restores the state it found, so the closed entry state survives the timeout"
    );
}

#[test]
fn a_strict_headless_wait_refuses_at_policy_on_the_first_poll() {
    crate::tree::fixture::bootstrap();
    let _fixture_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _cleanup = CloseCenterOnDrop;
    let _ = close_surface(SnapshotSurface::ActionCenter, deadline(8_000));
    assert!(
        wait_for_foreground_to_settle(),
        "the desktop's foreground must settle before the refusal is staged"
    );

    let started = Instant::now();
    let error = wait::execute(
        notification_wait_args(10_000),
        &WindowsAdapter::new(),
        &CommandContext::default(),
    )
    .expect_err("a strict-headless caller is refused at policy");
    let elapsed = started.elapsed();

    assert!(
        elapsed < Duration::from_secs(2),
        "the refusal must fire on the first poll, not burn the deadline: \
         took {elapsed:?} of a 10s wait"
    );
    let AppError::Adapter(error) = error else {
        panic!("the refusal is a structured adapter error");
    };
    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(
        !center_open(),
        "the refused wait must not have raised the center"
    );
}
