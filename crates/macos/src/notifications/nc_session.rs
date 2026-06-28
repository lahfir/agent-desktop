use agent_desktop_core::error::AdapterError;

pub(crate) fn close_session<T>(
    session: NcSession,
    result: Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    let close_result = session.close();
    match (result, close_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(err)) => Err(err),
        (Err(err), _) => Err(err),
    }
}

pub(crate) struct NcSession {
    was_already_open: bool,
    previous_app: Option<String>,
    closed: bool,
}

impl NcSession {
    pub(crate) fn open() -> Result<Self, AdapterError> {
        let previous_app = frontmost_app();
        let was_already_open = is_nc_open();
        if !was_already_open {
            open_nc()?;
            wait_for_nc_ready()?;
        }
        Ok(Self {
            was_already_open,
            previous_app,
            closed: false,
        })
    }

    pub(crate) fn close(mut self) -> Result<(), AdapterError> {
        let close_result = if self.was_already_open {
            Ok(())
        } else {
            close_nc()
        };
        if let Some(ref app) = self.previous_app {
            reactivate_app(app);
        }
        self.closed = true;
        close_result
    }
}

impl Drop for NcSession {
    fn drop(&mut self) {
        if self.closed {
            return;
        }
        if !self.was_already_open {
            if let Err(e) = close_nc() {
                tracing::warn!("Failed to close NC in Drop: {e}");
            }
        }
        if let Some(ref app) = self.previous_app {
            reactivate_app(app);
        }
    }
}

#[cfg(target_os = "macos")]
fn frontmost_app() -> Option<String> {
    let mut command = std::process::Command::new("/usr/bin/osascript");
    command.args([
        "-e",
        "tell application \"System Events\" to get name of first application process whose frontmost is true",
    ]);
    let output = crate::system::process::run_with_timeout(
        &mut command,
        "frontmost-app osascript",
        std::time::Duration::from_secs(2),
    )
    .ok()?;
    if output.status.success() {
        let name = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if name.is_empty() { None } else { Some(name) }
    } else {
        None
    }
}

#[cfg(not(target_os = "macos"))]
fn frontmost_app() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn reactivate_app(name: &str) {
    let script = format!("tell application {} to activate", applescript_string(name));
    let mut command = std::process::Command::new("/usr/bin/osascript");
    command.arg("-e").arg(script);
    if let Err(e) = crate::system::process::run_with_timeout(
        &mut command,
        "reactivate-app osascript",
        std::time::Duration::from_secs(1),
    ) {
        tracing::warn!("reactivate_app osascript failed for app {:?}: {e}", name);
    }
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        if matches!(ch, '\\' | '"') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped.push('"');
    escaped
}

#[cfg(not(target_os = "macos"))]
fn reactivate_app(_name: &str) {}

#[cfg(target_os = "macos")]
pub(super) fn nc_pid() -> Option<i32> {
    let mut command = std::process::Command::new("/usr/bin/pgrep");
    command.arg("-x").arg("NotificationCenter");
    let output = crate::system::process::run_with_timeout(
        &mut command,
        "pgrep NotificationCenter",
        std::time::Duration::from_secs(1),
    )
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

    let mut command = std::process::Command::new("/usr/bin/osascript");
    command.arg("-e").arg(script);
    if let Err(e) = crate::system::process::run_with_timeout(
        &mut command,
        "osascript open-nc",
        std::time::Duration::from_secs(2),
    ) {
        tracing::warn!("open_nc osascript failed: {e}");
    }
    std::thread::sleep(std::time::Duration::from_millis(500));
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::applescript_string;

    #[test]
    fn applescript_string_escapes_quotes_and_backslashes() {
        assert_eq!(
            applescript_string(r#"Bad \ "Name""#),
            r#""Bad \\ \"Name\"""#
        );
    }
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
