use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

use agent_desktop_core::{ActionOps, AdapterError, ErrorCode, InputOps, ObservationOps, SystemOps};

static HOME_LOCK: Mutex<()> = Mutex::new(());
static HOME_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) struct HomeGuard {
    lock: Option<std::sync::MutexGuard<'static, ()>>,
    previous: Option<std::ffi::OsString>,
    path: PathBuf,
}

impl HomeGuard {
    pub(crate) fn new() -> Self {
        let lock = HOME_LOCK.lock().unwrap();
        let path = std::env::temp_dir().join(format!(
            "agent-desktop-dispatch-test-{}-{}",
            std::process::id(),
            HOME_ID.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        let previous = std::env::var_os("AGENT_DESKTOP_HOME");
        unsafe { std::env::set_var("AGENT_DESKTOP_HOME", &path) };
        Self {
            lock: Some(lock),
            previous,
            path,
        }
    }

    pub(crate) fn path(&self) -> &std::path::Path {
        &self.path
    }
}

impl Drop for HomeGuard {
    fn drop(&mut self) {
        match self.previous.take() {
            Some(previous) => unsafe { std::env::set_var("AGENT_DESKTOP_HOME", previous) },
            None => unsafe { std::env::remove_var("AGENT_DESKTOP_HOME") },
        }
        self.lock.take();
    }
}

pub(crate) struct FailingOverlayAdapter;

impl ObservationOps for FailingOverlayAdapter {}
impl ActionOps for FailingOverlayAdapter {}
impl InputOps for FailingOverlayAdapter {}

impl SystemOps for FailingOverlayAdapter {
    fn update_cursor_overlay(
        &self,
        _control: &agent_desktop_core::CursorOverlayControl,
    ) -> Result<(), AdapterError> {
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "overlay teardown failed",
        ))
    }
}

/// Records background pointer deliveries and counts real cursor events so
/// routing tests can prove which path a command took.
pub(crate) struct BackgroundPointerAdapter {
    pub(crate) background: Mutex<
        Vec<(
            agent_desktop_core::WindowInfo,
            agent_desktop_core::MouseEvent,
        )>,
    >,
    pub(crate) real_mouse_events: Mutex<u32>,
}

impl BackgroundPointerAdapter {
    pub(crate) const WINDOW_ID: &'static str = "w-9555";

    pub(crate) fn new() -> Self {
        Self {
            background: Mutex::new(Vec::new()),
            real_mouse_events: Mutex::new(0),
        }
    }

    fn window() -> agent_desktop_core::WindowInfo {
        agent_desktop_core::WindowInfo {
            id: Self::WINDOW_ID.into(),
            title: "Code".into(),
            app: "Code".into(),
            pid: agent_desktop_core::ProcessId::new(4242),
            process_instance: Some("instance".into()),
            bounds: Some(agent_desktop_core::Rect {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            }),
            state: agent_desktop_core::WindowState::default(),
        }
    }
}

impl ObservationOps for BackgroundPointerAdapter {
    fn list_windows(
        &self,
        _filter: &agent_desktop_core::WindowFilter,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<Vec<agent_desktop_core::WindowInfo>, AdapterError> {
        Ok(vec![Self::window()])
    }
}

impl ActionOps for BackgroundPointerAdapter {}

impl InputOps for BackgroundPointerAdapter {
    fn mouse_event(
        &self,
        _event: agent_desktop_core::MouseEvent,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<(), AdapterError> {
        *self.real_mouse_events.lock().unwrap() += 1;
        Ok(())
    }

    fn background_mouse_event(
        &self,
        window: &agent_desktop_core::WindowInfo,
        event: agent_desktop_core::MouseEvent,
        _lease: &agent_desktop_core::InteractionLease,
    ) -> Result<agent_desktop_core::BackgroundPointerReport, AdapterError> {
        self.background
            .lock()
            .unwrap()
            .push((window.clone(), event));
        Ok(agent_desktop_core::BackgroundPointerReport::default())
    }
}

impl SystemOps for BackgroundPointerAdapter {
    fn acquire_interaction_lease(
        &self,
        deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::InteractionLease, AdapterError> {
        agent_desktop_core::InteractionLease::guarded(deadline, ())
    }

    fn resolve_window_strict(
        &self,
        _window: &agent_desktop_core::WindowInfo,
        _deadline: agent_desktop_core::Deadline,
    ) -> Result<agent_desktop_core::WindowInfo, AdapterError> {
        Ok(Self::window())
    }
}
