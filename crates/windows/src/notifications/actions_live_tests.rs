use agent_desktop_core::{
    Deadline, DeliverySemantics, DismissNotificationRequest, ErrorCode, InteractionLease,
    InteractionPolicy, NotificationFilter, NotificationIdentity, NotificationInfo, SnapshotSurface,
    SystemOps,
};

use super::{dismiss_notification, notification_action};
use crate::adapter::WindowsAdapter;
use crate::notifications::list::list_entries;
use crate::notifications::session::ActionCenterSession;
use crate::notifications::toast_support::{
    self, StagedToast, TOAST_BODY, TOAST_BODY_SECOND, TOAST_TITLE, TOAST_TITLE_SECOND,
};
use crate::system::raise_oracle::{responded_since, witness_desktop};
use crate::system::shell_surface::resolve_surface;
use crate::system::shell_surface_open::close_surface;
use crate::system::test_support::{
    SHELL_SURFACE_LOCK, or_skip_shell, wait_for_foreground_to_settle,
};

fn deadline(ms: u64) -> Deadline {
    Deadline::after(ms).expect("deadline")
}

fn headed() -> InteractionPolicy {
    InteractionPolicy::headed()
}

fn foreground() -> isize {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
    (unsafe { GetForegroundWindow() }) as isize
}

fn center_open() -> bool {
    resolve_surface(SnapshotSurface::ActionCenter, deadline(10_000))
        .expect("the desktop is readable")
        .is_some()
}

fn identity_of(info: &NotificationInfo) -> NotificationIdentity {
    NotificationIdentity {
        expected_app: Some(info.app_name.clone()),
        expected_title: Some(info.title.clone()),
    }
}

fn listed_infos(hwnd: isize) -> Vec<NotificationInfo> {
    list_entries(&NotificationFilter::default(), hwnd, deadline(20_000))
        .expect("the listing reads")
        .into_iter()
        .map(|entry| entry.info)
        .collect()
}

/// Resets the center, opens it and holds it open, stages one synthetic toast
/// into the open center, and waits until the listing observes it.
///
/// The center stays open for the whole body because the measured staging
/// behaviour on this host is that a toast joins the center only while the
/// center is open and is evicted by its next close. The staging guard and the
/// held session both travel back to the caller, so the sweep and the close run
/// when the test body exits - held session first, then the toast sweep. The
/// mutations under test adopt the already-open center without raising it, and
/// their restore-on-exit leaves it open for the caller's follow-up reads.
///
/// A desktop whose shell declines the open, or whose staging never produces
/// an entry, skips loudly instead: the helper prints why and returns None.
fn stage_exactly_one_toast_into_held_center()
-> Option<(ActionCenterSession, StagedToast, NotificationInfo)> {
    toast_support::clear_center(deadline(20_000));
    let witness = witness_desktop();
    let held = or_skip_shell(
        "action center open for toast staging",
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
        "the staged toast is the only entry after the reset"
    );
    let entry = listed.into_iter().next().expect("one entry");
    Some((held, staged, entry))
}

#[test]
fn every_notification_info_field_is_populated_as_macos_populates_it() {
    crate::tree::fixture::bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some((held, _staged, staged)) = stage_exactly_one_toast_into_held_center() else {
        return;
    };

    assert_eq!(
        staged.index, 1,
        "a lone entry carries the 1-based index the contract gives every entry"
    );
    assert_eq!(staged.title, TOAST_TITLE);
    assert_eq!(
        staged.body.as_deref(),
        Some(TOAST_BODY),
        "the body the toast carried is the body the listing reports"
    );
    assert!(
        !staged.app_name.is_empty(),
        "app_name is filled the way macOS fills it, from the entry's source attribution"
    );

    let json = serde_json::to_value(vec![staged.clone()]).expect("the listing serializes");
    let entry = json
        .as_array()
        .and_then(|entries| entries.first())
        .expect("the JSON carries the entry")
        .clone();
    assert!(entry["index"].is_u64());
    assert!(entry["app_name"].is_string() && entry["app_name"] != "");
    assert!(entry["title"].is_string());
    assert_eq!(entry["body"], TOAST_BODY);
    let actions_absent = entry.get("actions").is_none();
    let round_trip: Vec<NotificationInfo> =
        serde_json::from_value(json).expect("the JSON deserializes into NotificationInfo");
    assert_eq!(round_trip[0].index, staged.index);
    assert_eq!(round_trip[0].app_name, staged.app_name);
    assert_eq!(round_trip[0].title, staged.title);
    assert_eq!(round_trip[0].body, staged.body);
    assert_eq!(
        actions_absent,
        staged.actions.is_empty(),
        "empty actions serialize as absent, which is the macOS shape"
    );
    held.close().expect("the held session restores the surface");
}

#[test]
fn a_mismatched_identity_is_refused_and_the_entry_survives() {
    crate::tree::fixture::bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some((held, _staged, staged)) = stage_exactly_one_toast_into_held_center() else {
        return;
    };
    let wrong_identity = NotificationIdentity {
        expected_app: None,
        expected_title: Some("a title no staged entry carries".into()),
    };

    let error = dismiss_notification(
        staged.index,
        None,
        Some(&wrong_identity),
        headed(),
        deadline(30_000),
    )
    .expect_err("the surface reordered under the caller, so the index is not the identity");

    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert!(
        !error.message.contains(TOAST_TITLE),
        "the mismatch message is built from the index alone and never names the entry"
    );
    let remaining = listed_infos(held.hwnd());
    assert!(
        remaining.iter().any(|info| info.title == staged.title),
        "the entry the identity refused must survive untouched"
    );
    held.close().expect("the held session restores the surface");
}

#[test]
fn an_unknown_action_name_is_refused_and_leaves_the_entry_unchanged() {
    crate::tree::fixture::bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some((held, _staged, staged)) = stage_exactly_one_toast_into_held_center() else {
        return;
    };
    let identity = identity_of(&staged);
    let action_name = "agent-desktop-no-such-action";

    let error = notification_action(
        staged.index,
        Some(&identity),
        action_name,
        headed(),
        deadline(30_000),
    )
    .expect_err("a notification offering no such button is refused");

    assert_eq!(error.code, ErrorCode::ActionNotSupported);
    assert!(
        error.message.contains(action_name),
        "the refusal names the caller-supplied action name"
    );
    let remaining = listed_infos(held.hwnd());
    assert!(
        remaining.iter().any(|info| info.title == staged.title),
        "nothing was invoked, so the entry is unchanged"
    );
    held.close().expect("the held session restores the surface");
}

#[test]
fn a_dismiss_removes_exactly_the_identified_entry() {
    crate::tree::fixture::bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some((held, _staged, staged)) = stage_exactly_one_toast_into_held_center() else {
        return;
    };
    let identity = identity_of(&staged);

    let dismissed = dismiss_notification(
        staged.index,
        None,
        Some(&identity),
        headed(),
        deadline(30_000),
    )
    .expect("the identified entry dismisses and the removal verifies");

    assert_eq!(dismissed.title, staged.title);
    assert_eq!(dismissed.app_name, staged.app_name);
    let remaining = listed_infos(held.hwnd());
    assert!(
        remaining.iter().all(|info| info.title != staged.title),
        "the re-read proves the identified entry is gone"
    );
    held.close().expect("the held session restores the surface");
}

#[test]
fn an_accepted_dismiss_invoke_that_the_shell_ignores_is_action_failed_and_both_entries_survive() {
    crate::tree::fixture::bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some((held, _staged_a, staged)) = stage_exactly_one_toast_into_held_center() else {
        return;
    };
    let _staged_b = StagedToast::stage_with(TOAST_TITLE_SECOND, TOAST_BODY_SECOND);
    let listed = toast_support::wait_until_count_held(held.hwnd(), 2, deadline(30_000));
    assert_eq!(
        listed.len(),
        2,
        "the second staged toast is the only other entry"
    );
    let second = listed
        .iter()
        .find(|info| info.title == TOAST_TITLE_SECOND)
        .expect("the second toast is listed")
        .clone();
    let identity = identity_of(&second);

    let error = dismiss_notification(
        second.index,
        None,
        Some(&identity),
        headed(),
        deadline(30_000),
    )
    .expect_err("the shell accepts the dismiss invoke and ignores it, and with another entry present the clear-all control is no substitute");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    let remaining = listed_infos(held.hwnd());
    assert_eq!(remaining.len(), 2, "nothing was removed");
    assert!(remaining.iter().any(|info| info.title == staged.title));
    assert!(remaining.iter().any(|info| info.title == second.title));
    held.close().expect("the held session restores the surface");
}

#[test]
fn a_strict_headless_list_is_refused_and_the_center_is_not_opened() {
    crate::tree::fixture::bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let _cleanup = toast_support::CloseCenterOnDrop;
    let _ = close_surface(SnapshotSurface::ActionCenter, deadline(8_000));
    assert!(
        wait_for_foreground_to_settle(),
        "the desktop's foreground must settle before the refusal is staged"
    );
    let before = foreground();

    let adapter = WindowsAdapter::new();
    let error = SystemOps::list_notifications(
        &adapter,
        &NotificationFilter::default(),
        InteractionPolicy::headless(),
        deadline(10_000),
        None,
    )
    .expect_err("a strict-headless caller is refused");

    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert_eq!(
        foreground(),
        before,
        "a refusal that moved the foreground is not a refusal"
    );
    assert!(
        !center_open(),
        "the refused listing must not have raised the center"
    );
}

#[test]
fn the_dismiss_reaches_the_windows_override_not_the_trait_default() {
    let adapter = WindowsAdapter::new();
    let lease = InteractionLease::guarded(deadline(5_000), ()).expect("lease");
    let identity = NotificationIdentity::default();

    let error = SystemOps::dismiss_notification(
        &adapter,
        DismissNotificationRequest {
            index: 1,
            app_filter: None,
            identity: &identity,
            policy: InteractionPolicy::headless(),
        },
        &lease,
    )
    .expect_err("the headless floor refuses before anything is raised");

    assert_eq!(
        error.code,
        ErrorCode::PolicyDenied,
        "the trait default would answer PLATFORM_NOT_SUPPORTED instead"
    );
}

#[test]
fn a_headed_list_through_the_trait_adopts_the_open_center_without_closing_it() {
    crate::tree::fixture::bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    toast_support::clear_center(deadline(20_000));
    let witness = witness_desktop();
    let Some(held) = or_skip_shell(
        "action center open for the pre-arrangement",
        ActionCenterSession::open(headed(), deadline(15_000)),
        || responded_since(&witness),
    ) else {
        return;
    };
    let adapter = WindowsAdapter::new();

    let listed = SystemOps::list_notifications(
        &adapter,
        &NotificationFilter::default(),
        headed(),
        deadline(20_000),
        Some(&InteractionLease::guarded(deadline(5_000), ()).expect("lease")),
    )
    .expect("a headed observation carries its lease and adopts the open center");

    assert!(
        listed.is_empty(),
        "the center was reset; a leak is a staging bug"
    );
    assert!(
        center_open(),
        "the listing's session must restore the state it found: the center was open on entry and stays open"
    );
    held.close().expect("the held session restores the surface");
}
