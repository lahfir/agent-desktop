use agent_desktop_core::{
    action::WindowOp,
    adapter::{ActionOps, InputOps, ObservationOps, SystemOps},
    error::{AdapterError, ErrorCode},
    node::WindowInfo,
};
use std::sync::Mutex;

struct WindowIdentityAdapter {
    windows: Vec<WindowInfo>,
    last_window_op_id: Mutex<Option<String>>,
}

impl ObservationOps for WindowIdentityAdapter {}

impl ActionOps for WindowIdentityAdapter {}

impl InputOps for WindowIdentityAdapter {}

impl SystemOps for WindowIdentityAdapter {
    fn resolve_window_strict(&self, win: &WindowInfo) -> Result<WindowInfo, AdapterError> {
        let live = self
            .windows
            .iter()
            .find(|candidate| candidate.id == win.id)
            .cloned()
            .ok_or_else(|| {
                AdapterError::new(
                    ErrorCode::WindowNotFound,
                    format!("Window '{}' not found", win.id),
                )
            })?;
        if live.pid != win.pid {
            return Err(AdapterError::new(
                ErrorCode::WindowNotFound,
                format!("Window '{}' identity mismatch", win.id),
            ));
        }
        if !win.title.is_empty() && live.title != win.title {
            return Err(AdapterError::new(
                ErrorCode::WindowNotFound,
                format!("Window '{}' identity mismatch", win.id),
            ));
        }
        Ok(live)
    }

    fn window_op(&self, win: &WindowInfo, _op: WindowOp) -> Result<(), AdapterError> {
        let resolved = self.resolve_window_strict(win)?;
        *self.last_window_op_id.lock().unwrap() = Some(resolved.id);
        Ok(())
    }
}

fn untitled(id: &str, pid: i32) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: "Untitled".into(),
        app: "TextEdit".into(),
        pid,
        bounds: None,
        is_focused: false,
    }
}

#[test]
fn id_addressed_window_op_targets_matching_id_not_first_title_match() {
    let adapter = WindowIdentityAdapter {
        windows: vec![untitled("w-1", 10), untitled("w-2", 10)],
        last_window_op_id: Mutex::new(None),
    };
    let target = untitled("w-2", 10);

    SystemOps::window_op(&adapter, &target, WindowOp::Minimize).unwrap();

    assert_eq!(
        *adapter.last_window_op_id.lock().unwrap(),
        Some("w-2".into())
    );
}

#[test]
fn missing_id_returns_window_not_found() {
    let adapter = WindowIdentityAdapter {
        windows: vec![untitled("w-1", 10)],
        last_window_op_id: Mutex::new(None),
    };
    let target = untitled("w-999", 10);

    let err = SystemOps::window_op(&adapter, &target, WindowOp::Minimize).unwrap_err();

    assert_eq!(err.code, ErrorCode::WindowNotFound);
}

#[test]
fn recycled_id_with_wrong_pid_fails_closed() {
    let adapter = WindowIdentityAdapter {
        windows: vec![untitled("w-100", 99)],
        last_window_op_id: Mutex::new(None),
    };
    let target = untitled("w-100", 10);

    let err = adapter.resolve_window_strict(&target).unwrap_err();

    assert_eq!(err.code, ErrorCode::WindowNotFound);
}

#[test]
fn resolve_window_strict_default_is_not_supported() {
    struct StubSystemOps;
    impl ObservationOps for StubSystemOps {}
    impl ActionOps for StubSystemOps {}
    impl InputOps for StubSystemOps {}
    impl SystemOps for StubSystemOps {}

    let err = StubSystemOps
        .resolve_window_strict(&untitled("w-1", 10))
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::PlatformNotSupported);
}
