//! Source C's composition test: the Chromium DOM-menu arm added after A26-12
//! measured both shipped sources silent under a demonstrably open Chromium
//! context menu (a DOM menu inside the app's own window). The positive side
//! fires only on the real Chromium host the probe stages, so what is driven
//! here is the multi-pid composition against a real window.

#![cfg(target_os = "windows")]

use super::*;
use std::time::Duration;

use crate::system::fixture::test_support::{FIXTURE_APP_NAME_LOCK, settles_to};
use crate::tree::fixture::bootstrap;
use crate::tree::fixture::fixture_menu::MenuFixture;

const STATE_TIMEOUT: Duration = Duration::from_secs(5);

use crate::system::fixture::test_time::deadline;

/// The composition identity IS the invariant: for the same pid,
/// `menus_open_for` must answer exactly what [`menu_is_open`] answers,
/// including the Chromium source the single-pid composition gained. The
/// at-rest leg is the one that exercises source C inside the multi-pid path
/// on this box - classic and source B read false, so the shared Chromium arm
/// runs and must stay quiet for the framework-gate reason the arm
/// documents. A staged Chromium DOM menu is not feasible here (no
/// Chromium host is staged in the unit lane), so a firing source C is
/// covered by the single-pid arm's own measurement (A26-12) and by the arm
/// being the same shared function both compositions call; the popup leg
/// below proves the short-circuit order keeps the identity when an earlier
/// source fires.
///
/// Each leg first waits for both compositions to reach the state the fixture
/// is in, because the OS views they read move after the fixture's own state
/// does; the identity is then asserted on the settled pair, so one comparison
/// cannot straddle a composition that has flipped and one that has not.
#[test]
fn menus_open_for_answers_identically_to_menu_is_open_for_the_fixture_pid() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _stage = crate::tree::fixture::fixture_window::on_screen_stage();
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let pid = ProcessId::from(fixture.process_id());

    let composed = || {
        menus_open_for(&[pid], deadline(10_000))
            .expect("the multi-pid predicate reads the fixture")
            .get(&pid)
            .copied()
            .expect("the queried pid is in the answer")
    };
    let single =
        || menu_is_open(pid, deadline(10_000)).expect("the single-pid predicate reads the fixture");

    let agree_on = |expected: bool, context: &str| {
        assert!(
            settles_to(STATE_TIMEOUT, expected, composed),
            "menus_open_for never settled to {expected}: {context}"
        );
        assert!(
            settles_to(STATE_TIMEOUT, expected, single),
            "menu_is_open never settled to {expected}: {context}"
        );
        assert_eq!(composed(), single(), "{context}");
    };

    agree_on(
        false,
        "at rest both compositions probe every source and must agree",
    );

    fixture.open_context_menu();
    assert!(fixture.wait_for_menu_state(true, STATE_TIMEOUT));
    agree_on(
        true,
        "with the popup open the composition must answer what menu_is_open answers - an \
         earlier source firing keeps the Chromium probe out while the identity holds",
    );

    fixture.dismiss_context_menu();
    assert!(fixture.wait_for_menu_state(false, STATE_TIMEOUT));
    agree_on(
        false,
        "after dismissal the composition must return to the single-pid answer",
    );
}
