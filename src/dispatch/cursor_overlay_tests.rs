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
