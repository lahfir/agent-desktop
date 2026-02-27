use agent_desktop_core::error::AdapterError;

pub struct NcSession {
    was_already_open: bool,
}

impl NcSession {
    pub fn open() -> Result<Self, AdapterError> {
        let was_already_open = is_nc_open();
        if !was_already_open {
            open_nc()?;
            wait_for_nc_ready()?;
        }
        Ok(Self { was_already_open })
    }

    pub fn close(self) -> Result<(), AdapterError> {
        if !self.was_already_open {
            close_nc()?;
        }
        std::mem::forget(self);
        Ok(())
    }
}

impl Drop for NcSession {
    fn drop(&mut self) {
        if !self.was_already_open {
            if let Err(e) = close_nc() {
                tracing::warn!("Failed to close NC in Drop: {e}");
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub(super) fn nc_pid() -> Option<i32> {
    let output = std::process::Command::new("pgrep")
        .arg("-x")
        .arg("NotificationCenter")
        .output()
        .ok()?;

    String::from_utf8_lossy(&output.stdout)
        .trim()
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<i32>().ok())
}

#[cfg(target_os = "macos")]
fn is_nc_open() -> bool {
    use crate::tree::{copy_ax_array, element_for_pid};

    let pid = match nc_pid() {
        Some(p) => p,
        None => return false,
    };
    let app = element_for_pid(pid);
    let windows = copy_ax_array(&app, "AXWindows").unwrap_or_default();
    !windows.is_empty()
}

#[cfg(not(target_os = "macos"))]
fn is_nc_open() -> bool {
    false
}

#[cfg(target_os = "macos")]
fn open_nc() -> Result<(), AdapterError> {
    let script = r#"tell application "System Events" to tell its application process "ControlCenter"
        click (first menu bar item of menu bar 1 whose description is "Clock")
    end tell"#;

    let mut child = std::process::Command::new("osascript")
        .arg("-e")
        .arg(script)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| AdapterError::internal(format!("Failed to spawn osascript: {e}")))?;

    std::thread::sleep(std::time::Duration::from_millis(500));
    let _ = child.try_wait();
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn open_nc() -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("open_nc"))
}

#[cfg(target_os = "macos")]
fn close_nc() -> Result<(), AdapterError> {
    use crate::input::keyboard;
    use agent_desktop_core::action::KeyCombo;

    let combo = KeyCombo {
        key: "escape".into(),
        modifiers: vec![],
    };
    keyboard::synthesize_key(&combo)?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn close_nc() -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("close_nc"))
}

#[cfg(target_os = "macos")]
fn wait_for_nc_ready() -> Result<(), AdapterError> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    let poll = std::time::Duration::from_millis(50);

    loop {
        if is_nc_open() {
            return Ok(());
        }
        if std::time::Instant::now() > deadline {
            return Err(AdapterError::timeout(
                "Notification Center did not open within 2 seconds",
            ));
        }
        std::thread::sleep(poll);
    }
}

#[cfg(not(target_os = "macos"))]
fn wait_for_nc_ready() -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("wait_for_nc_ready"))
}
