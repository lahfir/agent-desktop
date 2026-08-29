//! Source C's own tests: the Chromium DOM-menu arm added after A26-12
//! measured both shipped sources silent under a demonstrably open Chromium
//! context menu (a DOM menu inside the app's own window). The positive side
//! fires only on the real Chromium host the probe stages, so what is driven
//! here is the measured negative contract against a real window: a permanent
//! Win32 menu bar under exactly this source's candidate pool must stay quiet,
//! which is the framework gate's invert-verification because dropping the
//! gate makes the bar's own `MenuItem` elements fire the predicate here, and
//! a classic tool-window popup that source B covers must never fire this
//! source.

#![cfg(target_os = "windows")]

use super::*;
use crate::system::test_support::settles_to;
use std::time::Duration;

use crate::system::test_support::FIXTURE_APP_NAME_LOCK;
use crate::tree::fixture::bootstrap;
use crate::tree::fixture_menu::MenuFixture;

const STATE_TIMEOUT: Duration = Duration::from_secs(5);

fn deadline() -> Deadline {
    Deadline::after(10_000).expect("bounded deadline")
}

#[test]
fn a_permanent_win32_menu_bar_never_fires_the_chromium_source() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let pid = ProcessId::from(fixture.process_id());

    let quiet = || chromium_dom_menu_reachable(pid, deadline()).expect("chromium probe reads it");

    assert!(
        !quiet(),
        "the fixture's main window is this source's exact candidate pool - a visible, non-tool, root-level window of the pid carrying a permanent Win32 menu bar whose own MenuItem elements would fire a bare menu-family search; the framework gate is what keeps the predicate at rest, and removing it fails this assertion"
    );
    assert!(
        !menu_is_open(pid, deadline()).expect("predicate reads the fixture"),
        "the composed predicate stays closed while only a menu bar exists"
    );

    fixture.open_context_menu();
    assert!(fixture.wait_for_menu_state(true, STATE_TIMEOUT));
    assert!(
        uia_menu_reachable(pid, deadline()).expect("uia probe reads the fixture"),
        "precondition: the fixture's open popup is in source B's pool"
    );
    assert!(
        settles_to(STATE_TIMEOUT, false, quiet),
        "a native classic popup is a tool-window menu, never a Chromium DOM menu"
    );

    fixture.dismiss_context_menu();
    assert!(fixture.wait_for_menu_state(false, STATE_TIMEOUT));
    assert!(
        settles_to(STATE_TIMEOUT, false, quiet),
        "the dismissed state must stay quiet"
    );
}

/// The composition identity IS the invariant: for the same pid,
/// `menus_open_for` must answer exactly what [`menu_is_open`] answers,
/// including the Chromium source the single-pid composition gained. The
/// at-rest leg is the one that exercises source C inside the multi-pid path
/// on this box - classic and source B read false, so the shared Chromium arm
/// runs and must stay quiet for the same framework-gate reason the
/// single-pid test pins. A staged Chromium DOM menu is not feasible here (no
/// Chromium host is staged in the unit lane), so a firing source C is
/// covered by the single-pid arm's own measurement (A26-12) and by the arm
/// being the same shared function both compositions call; the popup leg
/// below proves the short-circuit order keeps the identity when an earlier
/// source fires.
#[test]
fn menus_open_for_answers_identically_to_menu_is_open_for_the_fixture_pid() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let pid = ProcessId::from(fixture.process_id());

    let composed = || {
        menus_open_for(&[pid], deadline())
            .expect("the multi-pid predicate reads the fixture")
            .get(&pid)
            .copied()
            .expect("the queried pid is in the answer")
    };
    let single =
        || menu_is_open(pid, deadline()).expect("the single-pid predicate reads the fixture");

    assert_eq!(
        composed(),
        single(),
        "at rest both compositions probe every source and must agree"
    );

    fixture.open_context_menu();
    assert!(fixture.wait_for_menu_state(true, STATE_TIMEOUT));
    assert_eq!(
        composed(),
        single(),
        "with the popup open the composition must answer what menu_is_open answers - an \
         earlier source firing keeps the Chromium probe out while the identity holds"
    );

    fixture.dismiss_context_menu();
    assert!(fixture.wait_for_menu_state(false, STATE_TIMEOUT));
    assert_eq!(
        composed(),
        single(),
        "after dismissal the composition must return to the single-pid answer"
    );
}
