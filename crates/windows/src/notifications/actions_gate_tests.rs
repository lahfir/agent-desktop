//! The clear-all substitute's re-verify gate, driven live against the same
//! staged-toast arrangement the accepted-but-ignored test uses: the substitute
//! may only fire while the center still holds exactly one entry and it is
//! still the target, so the identity-mismatch case and the
//! two-entry-appears case must both answer `false` before anything is
//! invoked.

#![cfg(target_os = "windows")]

use agent_desktop_core::{Deadline, InteractionPolicy, NotificationInfo};

use super::clear_all_still_the_sole_target;
use crate::notifications::session::ActionCenterSession;
use crate::notifications::toast_support::{
    self, StagedToast, TOAST_BODY_SECOND, TOAST_TITLE_SECOND,
};
use crate::system::raise_oracle::{responded_since, witness_desktop};
use crate::system::test_support::{SHELL_SURFACE_LOCK, or_skip_shell};

fn deadline(ms: u64) -> Deadline {
    Deadline::after(ms).expect("deadline")
}

fn headed() -> InteractionPolicy {
    InteractionPolicy::headed()
}

/// With exactly one staged entry and its own identity the substitute is
/// faithful and the gate answers true; a fabricated identity the sole entry
/// does not carry aborts it; a second entry arriving aborts it - the
/// raced-arrival shape the gate exists to close.
#[test]
fn the_clear_all_substitute_fires_only_on_exactly_one_matching_entry() {
    crate::tree::fixture::bootstrap();
    let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    toast_support::clear_center(deadline(20_000));
    let witness = witness_desktop();
    let Some(held) = or_skip_shell(
        "action center open for the gate check",
        ActionCenterSession::open(headed(), deadline(15_000)),
        || responded_since(&witness),
    ) else {
        return;
    };
    let _staged = StagedToast::stage();
    let Some(listed) = toast_support::wait_until_listed_held(held.hwnd(), deadline(30_000)) else {
        eprintln!("skip toast staging: this desktop's toast staging produced no entry");
        held.close().expect("the held session restores the surface");
        return;
    };
    assert_eq!(
        listed.len(),
        1,
        "the staged toast is the only entry after the reset"
    );
    let target = listed.first().expect("one entry").clone();

    assert!(
        clear_all_still_the_sole_target(held.hwnd(), &target, deadline(20_000))
            .expect("the re-verify read"),
        "exactly one entry whose identity matches is the one shape the substitute may fire on"
    );

    let foreign = NotificationInfo {
        index: 1,
        app_name: target.app_name.clone(),
        title: format!("{} (not the staged entry)", target.title),
        body: None,
        actions: Vec::new(),
    };
    assert!(
        !clear_all_still_the_sole_target(held.hwnd(), &foreign, deadline(20_000))
            .expect("the re-verify read"),
        "an identity the sole entry does not carry must abort the substitute"
    );

    let _staged_b = StagedToast::stage_with(TOAST_TITLE_SECOND, TOAST_BODY_SECOND);
    let listed = toast_support::wait_until_count_held(held.hwnd(), 2, deadline(30_000));
    assert_eq!(
        listed.len(),
        2,
        "the second staged toast is the only other entry"
    );
    assert!(
        !clear_all_still_the_sole_target(held.hwnd(), &target, deadline(20_000))
            .expect("the re-verify read"),
        "an entry arriving after the settle read must abort the substitute - the \
         two-entry-appears case the substitute must never widen into"
    );
    held.close().expect("the held session restores the surface");
}
