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

    // The fixture's main window is this source's exact candidate pool - a
    // visible, non-tool, root-level window of the pid - and it carries a
    // permanent Win32 menu bar whose own MenuItem elements would fire a
    // bare menu-family search here. The framework gate is what keeps the
    // predicate at rest: removing it fails this assertion.
    assert!(
        !quiet(),
        "a Win32 menu bar at rest must not read as a Chromium menu open"
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
