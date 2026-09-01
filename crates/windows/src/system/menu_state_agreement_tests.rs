//! Whether the menu detector and the menu locator answer the same question.
//!
//! Split from `menu_state_tests.rs` so that file stays inside the size cap.
//! These cases are about the agreement between two predicates rather than
//! about either one on its own.

use super::*;

/// The defect this guards is a source set that drifts. `menu_is_open` composes
/// three sources; `locate_menu` implemented only one, so a wait that fired on
/// the Chromium source was followed by a snapshot that answered
/// `WINDOW_NOT_FOUND`. Adding a fourth source to the detector without giving
/// the locator a way to root it would reopen exactly that gap, and a live test
/// cannot see it: it needs the application that fires the new source.
///
/// The bodies are sliced rather than the files searched, because a first draft
/// of this test matched a bare mention of the Chromium locator and passed with
/// the call to it removed - a test that could not fail.
#[test]
fn every_detector_source_is_either_locatable_or_named_as_not() {
    let detector = include_str!("menu_state.rs");
    let locator = include_str!("menu_state_locate.rs");

    for source in [
        "classic_menu_mode_active",
        "uia_menu_reachable",
        "chromium_dom_menu_reachable",
    ] {
        assert!(
            detector.contains(source),
            "{source} is still one of the detector's sources"
        );
    }

    let locate_menu_body = body_of(locator, "pub(crate) fn locate_menu(");
    assert!(
        locate_menu_body.contains("probe_candidate_element(&client, &condition, handle)"),
        "locate_menu resolves the tool-window source"
    );
    assert!(
        locate_menu_body.contains("locate_chromium(pid, deadline)"),
        "locate_menu must reach the Chromium source, or a wait that fires on it is followed          by a snapshot that cannot root what the wait found"
    );

    let chromium_body = body_of(locator, "fn locate_chromium(");
    assert!(
        chromium_body.contains("locate_chromium_dom_menu(pid, deadline)"),
        "the Chromium fall-through actually calls the Chromium locator"
    );

    assert!(
        locator.contains("classic_menu_mode_active"),
        "the one source the locator cannot resolve is named in its own documentation, so the          divergence is stated rather than discovered"
    );
}

/// The text of one function, from its signature to the first line that closes
/// it at column zero. Enough to tell a call inside a body from a mention
/// elsewhere in the file, which is the distinction the test above needs.
fn body_of<'a>(source: &'a str, signature: &str) -> &'a str {
    let start = source
        .find(signature)
        .unwrap_or_else(|| panic!("{signature} still exists"));
    let rest = &source[start..];
    let end = rest
        .find(
            "
}",
        )
        .map_or(rest.len(), |offset| offset + 2);
    &rest[..end]
}

/// At rest the two predicates agree, in both directions, against a real
/// fixture: nothing is open and nothing can be rooted. This is the live half -
/// it cannot exercise the Chromium source, which needs a Chromium application,
/// but it does prove the locator's new fall-through did not make it answer
/// `Some` for a process with no menu at all.
#[test]
fn at_rest_the_detector_and_the_locator_agree_that_no_menu_is_open() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let pid = ProcessId::from(fixture.process_id());

    assert!(!menu_is_open(pid, deadline()).expect("the detector reads the fixture"));
    assert!(
        crate::system::menu_state::locate_menu(pid, deadline())
            .expect("the locator reads the fixture")
            .is_none(),
        "with no menu open the locator must answer None rather than rooting something else"
    );
}

/// The other direction for the tool-window source, which this fixture can
/// stage: an open menu is both detected and locatable, so the pair agrees
/// where the documentation says it must.
#[test]
fn an_open_tool_window_menu_is_both_detected_and_locatable() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let fixture = MenuFixture::spawn().expect("the menu fixture starts");
    let pid = ProcessId::from(fixture.process_id());

    fixture.open_context_menu();
    assert!(fixture.wait_for_menu_state(true, STATE_TIMEOUT));
    assert!(settles_to(STATE_TIMEOUT, true, || menu_is_open(
        pid,
        deadline()
    )
    .expect("the detector reads the fixture")));

    assert!(
        settles_to(STATE_TIMEOUT, true, || {
            crate::system::menu_state::locate_menu(pid, deadline())
                .expect("the locator reads the fixture")
                .is_some()
        }),
        "a menu the detector reports open must be one the surface can root"
    );

    fixture.dismiss_context_menu();
    assert!(fixture.wait_for_menu_state(false, STATE_TIMEOUT));
}

/// against pid reuse from a spawned-and-killed process.
#[test]
fn a_nonexistent_pid_returns_a_classified_error_not_a_panic_or_false_closed() {
    let pid = ProcessId::from(1u32);

    let error = menu_is_open(pid, deadline())
        .expect_err("a nonexistent pid must not silently report closed");

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
}
