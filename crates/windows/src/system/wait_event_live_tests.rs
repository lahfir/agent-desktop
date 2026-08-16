//! Windows-only: drives `agent_desktop_core::commands::wait::execute` end to
//! end against real fixtures for the five non-surface `--event` tokens, plus
//! the `window-opened` form of the unnamed-surface-discovery proof: a modal
//! dialog opened mid-wait is reported without the caller supplying its
//! title or id. Every capture and diff below runs through the real
//! `diff_signals` - never a hand-built baseline - so this is the layer above
//! `signals_tests.rs` / `signal_inventory_tests.rs`, which exercise the
//! adapter method directly rather than the command a caller actually runs.
//!
//! Every fixture spawned anywhere in this test binary shares one process
//! image name - `HostedFixture`/`ModalFixture` both re-exec
//! `std::env::current_exe()` verbatim - so a wait that resolved its `--app`
//! scope by asking the desktop "who has this name" (`resolve_app`, backed by
//! `list_apps`) would collide with any other fixture-spawning test running
//! concurrently anywhere in this binary and fail `AMBIGUOUS_TARGET`. Every
//! test below except `app-launched` instead seeds `CommandContext` with a
//! `capture_signal_baseline` already scoped to this fixture's own
//! `(pid, process_instance)` (`context::with_event_baseline`); core's
//! `signal_filter` resolves `--app` from that seed via `process_from_baseline`
//! rather than calling `resolve_app` (`wait_event.rs:120-131`), and a
//! process-filtered capture can only ever contain this fixture, so the
//! ambiguity class does not arise regardless of what else is running.
//! `window-closed` and `focus-changed` narrow by the fixture's own
//! `--window-id` instead and never resolve `--app` at all. `app-launched`
//! has no seed to resolve against - the process does not exist yet - so its
//! filter always scans by name across the whole population
//! (`wait_event.rs:113-118`); it cannot itself raise `AMBIGUOUS_TARGET` (that
//! branch never calls `resolve_app`), but an unrelated same-named fixture
//! launching or exiting during its poll window could produce a same-shaped
//! match, so it still holds `test_support::FIXTURE_APP_NAME_LOCK` to bound
//! that residual case to fixtures this suite itself does not otherwise
//! serialize against.
#![cfg(target_os = "windows")]

use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

use agent_desktop_core::commands::wait::{self, WaitArgs, WaitModeArgs, WaitPredicateArgs};
use agent_desktop_core::{
    AppError, CommandContext, Deadline, InteractionLease, ProcessId, ProcessIdentity,
    SignalBaseline, SignalFilter, SystemOps, WindowInfo, WindowState, diff_signals,
};
use serde_json::Value;

use crate::adapter::WindowsAdapter;
use crate::system::process_identity;
use crate::system::test_support::FIXTURE_APP_NAME_LOCK;
use crate::system::window_activate::focus_window;
use crate::tree::fixture::{HostedFixture, bootstrap};
use crate::tree::fixture_modal::ModalFixture;

const SETTLE: Duration = Duration::from_millis(250);
const TRANSITION_AFTER: Duration = Duration::from_millis(400);
const RECV_TIMEOUT: Duration = Duration::from_secs(20);
const FIXTURE_STATE_TIMEOUT: Duration = Duration::from_secs(5);
const WAIT_TIMEOUT_MS: u64 = 10_000;

fn own_image_name() -> String {
    std::env::current_exe()
        .expect("this test binary has a resolvable path")
        .file_name()
        .expect("the executable path carries a file name")
        .to_string_lossy()
        .into_owned()
}

fn process_identity_for(pid: u32) -> ProcessIdentity {
    let process = ProcessId::from(pid);
    let token = process_identity::token_for_pid(process)
        .expect("a live process's token is readable")
        .expect("a live process has a readable generation token");
    ProcessIdentity::new(process, token)
}

/// A real, process-scoped capture - never a hand-built baseline - taken at
/// the moment the caller chooses, so it reflects exactly the "before" state
/// each test needs (a window not yet open, a surface not yet visible, a
/// still-alive process about to terminate). Because it is scoped to one
/// exact `(pid, process_instance)`, it can contain at most this one
/// fixture regardless of anything else running concurrently.
fn scoped_baseline(identity: &ProcessIdentity) -> SignalBaseline {
    let filter = SignalFilter {
        app: None,
        process: Some(identity.clone()),
    };
    WindowsAdapter::new()
        .capture_signal_baseline(&filter, Deadline::after(5_000).expect("bounded deadline"))
        .expect("a process-filtered capture on a live desktop must succeed")
}

struct EventWaitScope {
    app: Option<String>,
    window_id: Option<String>,
    seed: Option<SignalBaseline>,
}

fn event_wait(event: &str, scope: EventWaitScope) -> Result<Value, AppError> {
    let args = WaitArgs {
        mode: WaitModeArgs {
            event: Some(event.to_string()),
            window_id: scope.window_id,
            ..WaitModeArgs::default()
        },
        predicate: WaitPredicateArgs {
            snapshot_id: None,
            predicate: None,
            value: None,
            action: None,
            count: None,
        },
        timeout_ms: WAIT_TIMEOUT_MS,
        app: scope.app,
    };
    let context = CommandContext::default().with_event_baseline(scope.seed.map(Ok));
    let adapter = WindowsAdapter::new();
    wait::execute(args, &adapter, &context)
}

fn spawn_event_wait(
    event: &'static str,
    scope: EventWaitScope,
) -> std::sync::mpsc::Receiver<Result<Value, AppError>> {
    let (sender, receiver) = channel();
    thread::spawn(move || {
        let _ = sender.send(event_wait(event, scope));
    });
    receiver
}

fn found_event(outcome: Result<Value, AppError>) -> Value {
    let body = outcome.expect("the wait must find the requested event rather than time out");
    assert_eq!(body["found"], Value::Bool(true));
    assert!(
        body["elapsed_ms"].is_number(),
        "the success envelope must carry elapsed_ms, the shape macOS's own wait success \
         payload carries: {body}"
    );
    body["event"].clone()
}

fn window_info_for(fixture: &HostedFixture) -> WindowInfo {
    let identity = process_identity_for(fixture.process_id());
    WindowInfo {
        id: format!("w-{}", fixture.handle() as usize),
        title: String::new(),
        app: String::new(),
        pid: identity.pid,
        process_instance: Some(identity.instance),
        bounds: None,
        state: WindowState::default(),
    }
}

fn lease() -> InteractionLease {
    InteractionLease::guarded(Deadline::after(10_000).expect("bounded deadline"), ())
        .expect("a lease can be acquired for the test")
}

/// A Windows dialog is a real top-level HWND, so `window-opened` must
/// discover it without the caller ever naming its title or id - the
/// property this test exists to prove.
#[test]
fn window_opened_fires_for_a_modal_opened_mid_wait_with_no_title_or_id_supplied() {
    bootstrap();
    let fixture = ModalFixture::spawn().expect("the modal fixture starts");
    let identity = process_identity_for(fixture.process_id());
    let seed = scoped_baseline(&identity);
    let receiver = spawn_event_wait(
        "window-opened",
        EventWaitScope {
            app: Some(own_image_name()),
            window_id: None,
            seed: Some(seed),
        },
    );

    thread::sleep(TRANSITION_AFTER);
    fixture.open();
    assert!(fixture.wait_for_modal_state(true, FIXTURE_STATE_TIMEOUT));

    let outcome = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("the wait completed");
    let event = found_event(outcome);
    assert!(
        event["window_id"].is_string(),
        "the server must supply the new window's identity; the caller named none: {event}"
    );
}

#[test]
fn window_closed_fires_when_the_fixtures_window_closes_mid_wait() {
    bootstrap();
    let mut fixture = HostedFixture::spawn().expect("a fixture host starts");
    let window_id = format!("w-{}", fixture.handle() as usize);
    let receiver = spawn_event_wait(
        "window-closed",
        EventWaitScope {
            app: None,
            window_id: Some(window_id),
            seed: None,
        },
    );

    thread::sleep(TRANSITION_AFTER);
    fixture.terminate();

    let outcome = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("the wait completed");
    found_event(outcome);
}

#[test]
fn app_launched_fires_when_the_fixture_process_starts_mid_wait() {
    bootstrap();
    let _app_name_scope = FIXTURE_APP_NAME_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let receiver = spawn_event_wait(
        "app-launched",
        EventWaitScope {
            app: Some(own_image_name()),
            window_id: None,
            seed: None,
        },
    );

    thread::sleep(TRANSITION_AFTER);
    let fixture = HostedFixture::spawn().expect("a fixture host starts");

    let outcome = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("the wait completed");
    found_event(outcome);
    drop(fixture);
}

#[test]
fn app_terminated_fires_when_the_fixture_process_exits_mid_wait() {
    bootstrap();
    let mut fixture = HostedFixture::spawn().expect("a fixture host starts");
    let identity = process_identity_for(fixture.process_id());
    let seed = scoped_baseline(&identity);
    let receiver = spawn_event_wait(
        "app-terminated",
        EventWaitScope {
            app: Some(own_image_name()),
            window_id: None,
            seed: Some(seed),
        },
    );

    thread::sleep(TRANSITION_AFTER);
    fixture.terminate();

    let outcome = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("the wait completed");
    found_event(outcome);
}

/// Forces OS foreground onto the fixture's window and never restores it, so
/// this test holds `fixture_window::on_screen_stage` for its whole body -
/// declared before the fixture itself so Rust's reverse drop order keeps the
/// lock held through the fixture's teardown handoff, not just its explicit
/// focus call. The same guard `window_ops_tests.rs`'s focused-filter test,
/// `window_activate_tests.rs`, and `signals_tests.rs`'s composed-capture test
/// already take.
#[test]
fn focus_changed_fires_when_the_fixtures_window_is_focused_mid_wait() {
    bootstrap();
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let fixture = HostedFixture::spawn().expect("a fixture host starts");
    let window_id = format!("w-{}", fixture.handle() as usize);
    let receiver = spawn_event_wait(
        "focus-changed",
        EventWaitScope {
            app: None,
            window_id: Some(window_id),
            seed: None,
        },
    );

    thread::sleep(TRANSITION_AFTER);
    focus_window(&window_info_for(&fixture), &lease())
        .expect("the fixture's window must be forced to the foreground");

    let outcome = receiver
        .recv_timeout(RECV_TIMEOUT)
        .expect("the wait completed");
    found_event(outcome);
}

/// The negative half: a steady fixture, captured twice with no action taken
/// between the two, must produce none of the five non-surface event kinds -
/// including `FocusChangedWindow`, which this assertion deliberately does not
/// filter out. This is what catches an implementation that emits events from
/// instability rather than from an actual change - the failure mode a
/// positive-only suite structurally cannot see.
///
/// The fixture itself is scoped by `(pid, process_instance)`
/// (`scoped_baseline`), so it needs no `FIXTURE_APP_NAME_LOCK`. But a sibling
/// test elsewhere in this crate can hand OS foreground around between the two
/// captures below as part of its own teardown, and that is a real focus
/// change this test's own predicate has no way to distinguish from a bug -
/// asserting it away would be a semantic weakening, not an isolation fix. It
/// holds `fixture_window::on_screen_stage` across both captures instead, the
/// same DESKTOP/FOREGROUND-state guard `window_ops_tests.rs`'s focused-filter
/// test takes even though it stages nothing itself.
#[test]
fn two_captures_with_no_transition_on_a_steady_fixture_produce_no_event_of_any_kind() {
    bootstrap();
    let _stage = crate::tree::fixture_window::on_screen_stage();
    let fixture = HostedFixture::spawn().expect("a fixture host starts");
    let identity = process_identity_for(fixture.process_id());
    let before = scoped_baseline(&identity);
    thread::sleep(SETTLE);
    let after = scoped_baseline(&identity);

    let events = diff_signals(&before, &after);
    assert!(
        events.is_empty(),
        "a steady fixture captured twice with no change must produce no event: {events:?}"
    );
}
