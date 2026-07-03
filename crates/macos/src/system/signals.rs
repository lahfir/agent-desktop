use agent_desktop_core::{
    adapter::{SnapshotSurface, WindowFilter},
    error::AdapterError,
    signals::{DesktopSignal, SignalBaseline},
};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
pub fn supported_surfaces_impl() -> Vec<SnapshotSurface> {
    vec![
        SnapshotSurface::Window,
        SnapshotSurface::Focused,
        SnapshotSurface::Menu,
        SnapshotSurface::Menubar,
        SnapshotSurface::Sheet,
        SnapshotSurface::Popover,
        SnapshotSurface::Alert,
    ]
}

#[cfg(not(target_os = "macos"))]
pub fn supported_surfaces_impl() -> Vec<SnapshotSurface> {
    vec![SnapshotSurface::Window]
}

#[cfg(target_os = "macos")]
pub fn capture_signal_baseline_impl() -> Result<SignalBaseline, AdapterError> {
    let windows = crate::system::window_list::list_windows_impl(&WindowFilter {
        focused_only: false,
        app: None,
    })?;
    let focused = windows.iter().find(|w| w.is_focused);
    Ok(SignalBaseline {
        focused_app: focused.map(|w| w.app.clone()),
        focused_window_title: focused.map(|w| w.title.clone()),
        window_count: windows.len(),
    })
}

#[cfg(not(target_os = "macos"))]
pub fn capture_signal_baseline_impl() -> Result<SignalBaseline, AdapterError> {
    Err(AdapterError::not_supported("capture_signal_baseline"))
}

#[cfg(target_os = "macos")]
pub fn wait_for_signal_impl(
    baseline: &SignalBaseline,
    signal: &DesktopSignal,
    timeout_ms: u64,
) -> Result<DesktopSignal, AdapterError> {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    loop {
        if let Some(observed) = observe_signal(baseline, signal)? {
            return Ok(observed);
        }
        if Instant::now() >= deadline {
            return Err(AdapterError::timeout(format!(
                "Signal {:?} did not occur within {timeout_ms}ms",
                signal_kind(signal)
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[cfg(not(target_os = "macos"))]
pub fn wait_for_signal_impl(
    _baseline: &SignalBaseline,
    _signal: &DesktopSignal,
    _timeout_ms: u64,
) -> Result<DesktopSignal, AdapterError> {
    Err(AdapterError::not_supported("wait_for_signal"))
}

#[cfg(target_os = "macos")]
fn observe_signal(
    baseline: &SignalBaseline,
    expected: &DesktopSignal,
) -> Result<Option<DesktopSignal>, AdapterError> {
    let current = capture_signal_baseline_impl()?;
    match expected {
        DesktopSignal::AppActivated { app } => {
            if current.focused_app.as_deref() == Some(app.as_str())
                && current.focused_app != baseline.focused_app
            {
                return Ok(Some(DesktopSignal::AppActivated { app: app.clone() }));
            }
        }
        DesktopSignal::WindowFocused { window_id, title } => {
            let windows = crate::system::window_list::list_windows_impl(&WindowFilter {
                focused_only: true,
                app: None,
            })?;
            if let Some(win) = windows.first() {
                if win.id == *window_id || win.title == *title {
                    return Ok(Some(DesktopSignal::WindowFocused {
                        window_id: win.id.clone(),
                        title: win.title.clone(),
                    }));
                }
            }
        }
        DesktopSignal::WindowClosed { window_id } => {
            if current.window_count < baseline.window_count {
                let windows = crate::system::window_list::list_windows_impl(&WindowFilter {
                    focused_only: false,
                    app: None,
                })?;
                if !windows.iter().any(|w| w.id == *window_id) {
                    return Ok(Some(DesktopSignal::WindowClosed {
                        window_id: window_id.clone(),
                    }));
                }
            }
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn signal_kind(signal: &DesktopSignal) -> &'static str {
    match signal {
        DesktopSignal::AppActivated { .. } => "app_activated",
        DesktopSignal::WindowFocused { .. } => "window_focused",
        DesktopSignal::WindowClosed { .. } => "window_closed",
    }
}
