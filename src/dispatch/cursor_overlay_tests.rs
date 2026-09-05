use super::*;
use crate::cli_args::cursor_overlay_enable::CursorOverlayEnableArgs;
use crate::cli_args::cursor_overlay_style::CursorOverlayStyleArgs;
use crate::test_noop_ops::NoopAdapter;
use agent_desktop_core::commands::session::{self, SessionAction};
use agent_desktop_core::{
    ActionOps, AdapterError, InputOps, ObservationOps, SystemOps, context::CommandContext,
};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static HOME_LOCK: Mutex<()> = Mutex::new(());
static HOME_ID: AtomicU64 = AtomicU64::new(1);

struct IsolatedHome {
    _lock: std::sync::MutexGuard<'static, ()>,
    dir: std::path::PathBuf,
    previous: Option<std::ffi::OsString>,
}

impl IsolatedHome {
    fn enter() -> Self {
        let lock = HOME_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let id = HOME_ID.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "agent-desktop-cursor-overlay-test-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("create isolated state root");
        let previous = std::env::var_os("AGENT_DESKTOP_HOME");
        unsafe { std::env::set_var("AGENT_DESKTOP_HOME", &dir) };
        Self {
            _lock: lock,
            dir,
            previous,
        }
    }

    fn start_session(&self) -> String {
        let started = session::execute(SessionAction::Start {
            name: None,
            no_trace: true,
            screenshots: false,
        })
        .expect("session start");
        started["session_id"]
            .as_str()
            .expect("session id")
            .to_owned()
    }
}

impl Drop for IsolatedHome {
    fn drop(&mut self) {
        match self.previous.as_ref() {
            Some(previous) => unsafe { std::env::set_var("AGENT_DESKTOP_HOME", previous) },
            None => unsafe { std::env::remove_var("AGENT_DESKTOP_HOME") },
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

enum RenderOutcome {
    Succeed,
    Fail,
}

struct RenderingAdapter {
    outcome: RenderOutcome,
}

impl ObservationOps for RenderingAdapter {}
impl ActionOps for RenderingAdapter {}
impl InputOps for RenderingAdapter {}

impl SystemOps for RenderingAdapter {
    fn update_cursor_overlay(
        &self,
        _control: &agent_desktop_core::CursorOverlayControl,
    ) -> Result<(), AdapterError> {
        match self.outcome {
            RenderOutcome::Succeed => Ok(()),
            RenderOutcome::Fail => Err(AdapterError::internal("renderer unavailable")),
        }
    }
}

fn enable_args() -> CursorOverlayArgs {
    CursorOverlayArgs {
        action: CursorOverlayAction::Enable(CursorOverlayEnableArgs {
            label: None,
            max_words: None,
            style: CursorOverlayStyleArgs::default(),
        }),
    }
}

fn disable_args() -> CursorOverlayArgs {
    CursorOverlayArgs {
        action: CursorOverlayAction::Disable,
    }
}

#[test]
fn default_adapter_reports_rendered_false_on_enable() {
    let home = IsolatedHome::enter();
    let session_id = home.start_session();
    let context = CommandContext::new(Some(session_id), None, false).expect("context");

    let value = dispatch(enable_args(), &NoopAdapter, &context).expect("enable succeeds");

    assert_eq!(value["rendered"], false);
}

#[test]
fn overriding_adapter_reports_rendered_true_on_enable() {
    let home = IsolatedHome::enter();
    let session_id = home.start_session();
    let context = CommandContext::new(Some(session_id), None, false).expect("context");
    let adapter = RenderingAdapter {
        outcome: RenderOutcome::Succeed,
    };

    let value = dispatch(enable_args(), &adapter, &context).expect("enable succeeds");

    assert_eq!(value["rendered"], true);
}

#[test]
fn failing_adapter_reports_rendered_false_but_still_succeeds_on_enable() {
    let home = IsolatedHome::enter();
    let session_id = home.start_session();
    let context = CommandContext::new(Some(session_id), None, false).expect("context");
    let adapter = RenderingAdapter {
        outcome: RenderOutcome::Fail,
    };

    let value = dispatch(enable_args(), &adapter, &context).expect("enable stays fail-soft");

    assert_eq!(value["rendered"], false);
}

#[test]
fn disable_never_carries_a_rendered_field() {
    let home = IsolatedHome::enter();
    let session_id = home.start_session();
    let context = CommandContext::new(Some(session_id), None, false).expect("context");

    let default_value = dispatch(disable_args(), &NoopAdapter, &context).expect("disable succeeds");
    assert!(default_value.get("rendered").is_none());

    let rendering_adapter = RenderingAdapter {
        outcome: RenderOutcome::Succeed,
    };
    let rendering_value =
        dispatch(disable_args(), &rendering_adapter, &context).expect("disable succeeds");
    assert!(rendering_value.get("rendered").is_none());
}

/// Records the control the adapter was actually handed, which is the only
/// place the caller's label can be checked: the response envelope echoes the
/// label back from the session config whether or not the renderer ever
/// received it, so asserting on the envelope would pass with the label
/// dropped.
#[derive(Default)]
struct RecordingAdapter {
    seen: Mutex<Vec<agent_desktop_core::CursorOverlayControl>>,
}

impl ObservationOps for RecordingAdapter {}
impl ActionOps for RecordingAdapter {}
impl InputOps for RecordingAdapter {}

impl SystemOps for RecordingAdapter {
    fn update_cursor_overlay(
        &self,
        control: &agent_desktop_core::CursorOverlayControl,
    ) -> Result<(), AdapterError> {
        if let Ok(mut seen) = self.seen.lock() {
            seen.push(control.clone());
        }
        Ok(())
    }
}

fn labelled_enable_args(label: &str) -> CursorOverlayArgs {
    CursorOverlayArgs {
        action: CursorOverlayAction::Enable(CursorOverlayEnableArgs {
            label: Some(label.to_owned()),
            max_words: None,
            style: CursorOverlayStyleArgs::default(),
        }),
    }
}

/// The label the caller passed has to reach the renderer, not merely the
/// session config. It did not: this call site built the control from the
/// style alone, so every overlay drew the greeting while the envelope
/// reported the caller's own words back to them.
#[test]
fn the_callers_label_reaches_the_adapter_rather_than_only_the_envelope() {
    let home = IsolatedHome::enter();
    let session_id = home.start_session();
    let context = CommandContext::new(Some(session_id), None, false).expect("context");
    let adapter = RecordingAdapter::default();

    let value = dispatch(
        labelled_enable_args("Opening the file menu"),
        &adapter,
        &context,
    )
    .expect("enable succeeds");

    assert_eq!(value["rendered"], true);
    let seen = adapter.seen.lock().expect("recorded controls");
    let enable = seen.first().expect("the adapter was handed a control");
    assert_eq!(
        enable.label(),
        Some("Opening the file menu"),
        "the control handed to the renderer must carry what the caller asked to display"
    );
}

/// A caller who said nothing still gets the greeting, so the fix above did
/// not quietly remove the overlay's own announcement.
#[test]
fn an_enable_without_a_label_still_hands_the_renderer_the_greeting() {
    let home = IsolatedHome::enter();
    let session_id = home.start_session();
    let context = CommandContext::new(Some(session_id), None, false).expect("context");
    let adapter = RecordingAdapter::default();

    dispatch(enable_args(), &adapter, &context).expect("enable succeeds");

    let seen = adapter.seen.lock().expect("recorded controls");
    let enable = seen.first().expect("the adapter was handed a control");
    assert_eq!(
        enable.label(),
        Some(agent_desktop_core::CURSOR_OVERLAY_GREETING)
    );
}
