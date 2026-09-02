//! Teardown, proved by observing the desktop rather than by reading the
//! disable command's own answer.
//!
//! `cursor-overlay disable` returning `ok` is the claim under test, so it
//! cannot also be the evidence. Three independent observations stand in for
//! it: the renderer process is gone, the pipe name can be claimed again, and
//! the pixels the overlay painted are no longer on screen.
//!
//! The pipe observation is made through the product rather than by opening
//! the name directly. A test that composed the name itself could drift from
//! the renderer's own derivation and pass while the name was never released;
//! asking the CLI to enable again, and requiring a renderer with a *different*
//! process id, exercises the exact claim path a real caller depends on — a
//! held name makes the fresh child withdraw and the enable time out.

#![cfg(target_os = "windows")]

#[path = "screen_sample.rs"]
mod screen_sample;

use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

/// The overlay needs a desktop to draw on, and staging one is the same opt-in
/// the other on-screen suites use rather than a second variable to discover.
const LIVE_STAGE_VARIABLE: &str = "AGENT_DESKTOP_LIVE_WPF";

/// A colour nothing else on the desktop paints, so counting exact matches is
/// evidence instead of noise.
const ORACLE: (u8, u8, u8) = (0xFF, 0x00, 0xFF);
const ORACLE_HEX: &str = "#FF00FF";

const SETTLE: Duration = Duration::from_secs(6);

static SCRATCH_ID: AtomicU64 = AtomicU64::new(1);

/// These tests share one desktop, one oracle colour and one process table,
/// so they cannot run beside each other: one overlay's pixels satisfy
/// another's "still painted" wait and defeat its "torn down" wait. libtest
/// runs a target's tests concurrently by default, so the serialization has to
/// be here rather than assumed from a runner flag.
fn desktop() -> MutexGuard<'static, ()> {
    static DESKTOP: OnceLock<Mutex<()>> = OnceLock::new();
    match DESKTOP.get_or_init(|| Mutex::new(())).lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

struct Scratch {
    root: PathBuf,
    session: Mutex<Option<String>>,
    _desktop: MutexGuard<'static, ()>,
}

impl Scratch {
    fn create(label: &str) -> Self {
        let id = SCRATCH_ID.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "agent-desktop-overlay-{label}-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create scratch state root");
        Self {
            root,
            session: Mutex::new(None),
            _desktop: desktop(),
        }
    }

    /// Remembered so teardown reaps this test's own renderer and nothing
    /// else. An unscoped reap matches every overlay on the machine, including
    /// one a developer is looking at.
    fn owns(&self, session: &str) {
        if let Ok(mut held) = self.session.lock() {
            *held = Some(session.to_owned());
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let session = self
            .session
            .lock()
            .ok()
            .and_then(|held| held.clone())
            .unwrap_or_default();
        if session.is_empty() {
            let _ = std::fs::remove_dir_all(&self.root);
            return;
        }
        for id in overlay_children(&session) {
            let _ = Command::new("taskkill")
                .args(["/PID", &id.to_string(), "/F"])
                .output();
        }
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn skip_unless_live_staging(reason: &str) -> bool {
    if std::env::var_os(LIVE_STAGE_VARIABLE).is_none() {
        eprintln!(
            "skip {reason}: {LIVE_STAGE_VARIABLE} is unset here, so no on-screen staging was \
             authorised; the Test (Windows) CI lane sets it and owns executing this"
        );
        return true;
    }
    false
}

fn run(scratch: &Scratch, args: &[&str]) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
        .args(args)
        .env("AGENT_DESKTOP_HOME", &scratch.root)
        .output()
        .expect("the binary starts");
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "`{}` did not answer one JSON envelope ({error}): {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn start_session(scratch: &Scratch) -> String {
    let envelope = run(scratch, &["session", "start"]);
    assert_eq!(envelope["ok"], true, "the session must start: {envelope}");
    let id = envelope["data"]["session_id"]
        .as_str()
        .expect("a session id")
        .to_owned();
    scratch.owns(&id);
    id
}

fn enable(scratch: &Scratch, session: &str) -> serde_json::Value {
    run(
        scratch,
        &[
            "cursor-overlay",
            "enable",
            "--session",
            session,
            "--fill",
            ORACLE_HEX,
            "--rim",
            ORACLE_HEX,
        ],
    )
}

/// The renderer carries its session id in argv precisely so it can be found
/// from outside: it is a detached process with no console, no taskbar entry
/// and no window title to match on.
fn overlay_children(session: &str) -> Vec<u32> {
    let script = format!(
        "Get-CimInstance Win32_Process -Filter \"Name='agent-desktop.exe'\" | \
         Where-Object {{ $_.CommandLine -like '*--cursor-overlay-child*{session}*' }} | \
         ForEach-Object {{ $_.ProcessId }}"
    );
    let output = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .expect("powershell enumerates processes");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .collect()
}

/// Panics rather than answering zero when the screen cannot be read. A
/// capture fault reported as "no pixels" would satisfy every teardown wait on
/// its first poll, which is the failure this whole file exists to rule out.
fn oracle_pixels() -> usize {
    screen_sample::pixels_matching(ORACLE.0, ORACLE.1, ORACLE.2)
        .expect("the screen must be readable; a failed capture is not an empty screen")
}

/// Well inside one 1500ms idle tick, so the session watch cannot satisfy
/// these observations on the disable path's behalf.
///
/// The watch needs two consecutive readings and so reclaims in three to four
/// seconds; a budget of one whole tick would sit on its luckiest boundary
/// rather than clear of it. Measured at 26-62ms, so this is an order of
/// magnitude of headroom and still decisive.
const PROMPTLY: Duration = Duration::from_millis(600);

/// Waits for a condition instead of sleeping a fixed span and hoping. A
/// failure seen after a fixed sleep is a race, not a finding.
fn wait_until(what: &str, settled: impl FnMut() -> bool) {
    wait_within(SETTLE, what, settled);
}

fn wait_within(budget: Duration, what: &str, mut settled: impl FnMut() -> bool) {
    let deadline = Instant::now() + budget;
    while Instant::now() < deadline {
        if settled() {
            return;
        }
        std::thread::sleep(Duration::from_millis(120));
    }
    panic!("{what} did not happen within {budget:?}");
}

#[test]
fn disabling_leaves_no_process_no_held_name_and_no_pixels() {
    if skip_unless_live_staging("cursor overlay teardown") {
        return;
    }
    let scratch = Scratch::create("teardown");
    let session = start_session(&scratch);

    let enabled = enable(&scratch, &session);
    assert_eq!(enabled["ok"], true, "the overlay must enable: {enabled}");
    wait_until("the overlay painted its oracle colour", || {
        oracle_pixels() > 0
    });
    let children = overlay_children(&session);
    assert_eq!(
        children.len(),
        1,
        "exactly one renderer serves a session, got {children:?}"
    );
    let first = children[0];

    let disabled = run(
        &scratch,
        &["cursor-overlay", "disable", "--session", &session],
    );
    assert_eq!(disabled["ok"], true, "the overlay must disable: {disabled}");

    wait_within(PROMPTLY, "the renderer process ended", || {
        overlay_children(&session).is_empty()
    });
    wait_within(PROMPTLY, "the overlay pixels left the screen", || {
        oracle_pixels() == 0
    });

    let again = enable(&scratch, &session);
    assert_eq!(
        again["ok"], true,
        "a fresh enable must claim the released name: {again}"
    );
    wait_until("a replacement renderer started", || {
        !overlay_children(&session).is_empty()
    });
    let replacement = overlay_children(&session);
    assert_eq!(replacement.len(), 1, "still exactly one renderer");
    assert_ne!(
        replacement[0], first,
        "the replacement must be a new process; the same pid would mean nothing was torn down"
    );

    let _ = run(
        &scratch,
        &["cursor-overlay", "disable", "--session", &session],
    );
}

/// `session end` dispatches a disable of its own, so this covers that wiring
/// rather than the renderer's session watch. The watch is what the following
/// test exercises, and the two are deliberately separate: a single test
/// calling `session end` would pass on the dispatched disable alone and prove
/// nothing about an abandoned renderer.
#[test]
fn ending_the_session_through_the_cli_also_takes_the_overlay_down() {
    if skip_unless_live_staging("cursor overlay session-end reclaim") {
        return;
    }
    let scratch = Scratch::create("session-end");
    let session = start_session(&scratch);

    let enabled = enable(&scratch, &session);
    assert_eq!(enabled["ok"], true, "the overlay must enable: {enabled}");
    wait_until("the overlay painted its oracle colour", || {
        oracle_pixels() > 0
    });

    let ended = run(&scratch, &["session", "end", "--session", &session]);
    assert_eq!(ended["ok"], true, "the session must end: {ended}");

    wait_until("the abandoned renderer ended itself", || {
        overlay_children(&session).is_empty()
    });
    wait_until("the abandoned overlay left the screen", || {
        oracle_pixels() == 0
    });
}

/// The case nothing else covers: a session that ends without any `disable`
/// reaching the renderer — a crashed agent, a `session gc`, an operator who
/// simply stops. The manifest is marked ended in place, exactly as an
/// out-of-band ending leaves it, and no command is run.
///
/// The renderer has no console, no taskbar entry and no Alt-Tab presence, so
/// if it did not notice this for itself, nothing in the product could remove
/// it. Reclaim is bounded by the two consecutive readings the watch requires,
/// which is what stops one unreadable tick ending a healthy overlay.
#[test]
fn a_session_ended_out_of_band_reclaims_its_abandoned_renderer() {
    if skip_unless_live_staging("cursor overlay out-of-band reclaim") {
        return;
    }
    let scratch = Scratch::create("out-of-band");
    let session = start_session(&scratch);

    let enabled = enable(&scratch, &session);
    assert_eq!(enabled["ok"], true, "the overlay must enable: {enabled}");
    wait_until("the overlay painted its oracle colour", || {
        oracle_pixels() > 0
    });

    let manifest = scratch
        .root
        .join("sessions")
        .join(&session)
        .join(agent_desktop_core::session::SESSION_MANIFEST_FILE);
    let mut body: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&manifest).expect("the manifest is readable"),
    )
    .expect("the manifest parses");
    body["ended_at"] = serde_json::json!(1_788_000_000_000u64);
    std::fs::write(
        &manifest,
        serde_json::to_string_pretty(&body).expect("the manifest re-serializes"),
    )
    .expect("the manifest is writable");

    wait_until("the abandoned renderer ended itself", || {
        overlay_children(&session).is_empty()
    });
    wait_until("the abandoned overlay left the screen", || {
        oracle_pixels() == 0
    });
}

/// Renderers are per session, and the pipe name carries the session id. A
/// disable aimed at a different session must not reach this one, or one
/// agent's teardown would blank another's overlay.
#[test]
fn a_disable_for_another_session_leaves_this_overlay_alone() {
    if skip_unless_live_staging("cursor overlay session scoping") {
        return;
    }
    let scratch = Scratch::create("scoping");
    let mine = start_session(&scratch);
    let theirs = start_session(&scratch);

    let enabled = enable(&scratch, &mine);
    assert_eq!(enabled["ok"], true, "the overlay must enable: {enabled}");
    wait_until("the overlay painted its oracle colour", || {
        oracle_pixels() > 0
    });

    let elsewhere = run(
        &scratch,
        &["cursor-overlay", "disable", "--session", &theirs],
    );
    assert_eq!(
        elsewhere["ok"], true,
        "disabling a session with no renderer is not an error: {elsewhere}"
    );

    assert_eq!(
        overlay_children(&mine).len(),
        1,
        "the other session's disable must not have ended this renderer"
    );
    assert!(
        oracle_pixels() > 0,
        "the other session's disable must not have cleared this overlay"
    );

    let _ = run(&scratch, &["cursor-overlay", "disable", "--session", &mine]);
}
