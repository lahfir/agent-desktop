use agent_desktop_core::{
    adapter::WindowFilter,
    error::AdapterError,
    node::WindowInfo,
};

#[cfg(target_os = "macos")]
pub fn focus_window_impl(win: &WindowInfo) -> Result<(), AdapterError> {
    use std::process::Command;
    let script = format!(
        r#"tell application "System Events"
    set frontmostProc to first process whose unix id is {}
    set frontmost of frontmostProc to true
end tell"#,
        win.pid
    );
    Command::new("osascript")
        .arg("-e")
        .arg(script)
        .output()
        .map_err(|e| AdapterError::internal(format!("focus_window failed: {e}")))?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn focus_window_impl(_win: &WindowInfo) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("focus_window"))
}

#[cfg(target_os = "macos")]
pub fn launch_app_impl(id: &str, wait: bool) -> Result<WindowInfo, AdapterError> {
    use crate::adapter::list_windows_impl;
    use std::process::Command;
    use std::time::{Duration, Instant};

    if id.contains("..") || id.starts_with('/') {
        return Err(AdapterError::new(
            agent_desktop_core::error::ErrorCode::InvalidArgs,
            format!("Invalid app identifier: '{id}'"),
        ));
    }

    Command::new("open")
        .arg("-a")
        .arg(id)
        .output()
        .map_err(|e| AdapterError::internal(format!("open failed: {e}")))?;

    if wait {
        let start = Instant::now();
        let timeout = Duration::from_secs(10);
        loop {
            std::thread::sleep(Duration::from_millis(200));
            let filter = WindowFilter { focused_only: false, app: Some(id.to_string()) };
            if let Ok(wins) = list_windows_impl(&filter) {
                if let Some(win) = wins.into_iter().next() {
                    return Ok(win);
                }
            }
            if start.elapsed() > timeout {
                break;
            }
        }
        return Err(AdapterError::new(
            agent_desktop_core::error::ErrorCode::AppNotFound,
            format!("App '{id}' launched but no window found within timeout"),
        )
        .with_suggestion(
            "Try again with a longer timeout or check that the app has a visible window",
        ));
    }

    std::thread::sleep(std::time::Duration::from_millis(500));
    let filter = WindowFilter { focused_only: false, app: Some(id.to_string()) };
    if let Ok(wins) = list_windows_impl(&filter) {
        if let Some(win) = wins.into_iter().next() {
            return Ok(win);
        }
    }
    Err(AdapterError::internal(format!("App '{id}' launched but no window found")))
}

#[cfg(not(target_os = "macos"))]
pub fn launch_app_impl(_id: &str, _wait: bool) -> Result<WindowInfo, AdapterError> {
    Err(AdapterError::not_supported("launch_app"))
}

#[cfg(target_os = "macos")]
pub fn close_app_impl(id: &str, force: bool) -> Result<(), AdapterError> {
    use std::process::Command;
    if force {
        Command::new("pkill")
            .arg("-x")
            .arg(id)
            .output()
            .map_err(|e| AdapterError::internal(format!("pkill failed: {e}")))?;
    } else {
        let safe_name = id.replace('"', "");
        let script = format!(
            r#"tell application "System Events"
    set theProc to first process whose name is "{safe_name}"
    tell theProc to quit
end tell"#
        );
        Command::new("osascript")
            .arg("-e")
            .arg(script)
            .output()
            .map_err(|e| AdapterError::internal(format!("quit failed: {e}")))?;
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
pub fn close_app_impl(_id: &str, _force: bool) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("close_app"))
}
