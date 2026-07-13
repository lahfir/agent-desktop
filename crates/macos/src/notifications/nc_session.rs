use agent_desktop_core::{AdapterError, Deadline, KeyCombo};

pub(crate) fn close_session<T>(
    session: NcSession,
    result: Result<T, AdapterError>,
) -> Result<T, AdapterError> {
    let close_result = session.close();
    match (result, close_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(value), Err(close_err)) => {
            tracing::warn!(error = %close_err, "notification center close failed after a successful operation");
            Ok(value)
        }
        (Err(err), _) => Err(err),
    }
}

pub(crate) struct NcSession {
    pid: i32,
    was_already_open: bool,
    previous_app: Option<String>,
    closed: bool,
    deadline: Deadline,
}

impl NcSession {
    pub(crate) fn open(deadline: Deadline) -> Result<Self, AdapterError> {
        let previous_app = frontmost_app(deadline);
        let (was_already_open, pid) = match nc_pid(deadline)? {
            Some(pid) if is_nc_open(pid, deadline) => (true, pid),
            _ => {
                open_nc(deadline)?;
                (false, wait_for_nc_ready(deadline)?)
            }
        };
        Ok(Self {
            pid,
            was_already_open,
            previous_app,
            closed: false,
            deadline,
        })
    }

    pub(crate) fn pid(&self) -> i32 {
        self.pid
    }

    pub(crate) fn close(mut self) -> Result<(), AdapterError> {
        let close_result = if self.was_already_open {
            Ok(())
        } else {
            close_nc(self.deadline)
        };
        if let Some(ref app) = self.previous_app {
            reactivate_app(app, self.deadline);
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
            if let Err(e) = close_nc(self.deadline) {
                tracing::warn!("Failed to close NC in Drop: {e}");
            }
        }
        if let Some(ref app) = self.previous_app {
            reactivate_app(app, self.deadline);
        }
    }
}

#[cfg(target_os = "macos")]
fn frontmost_app(deadline: Deadline) -> Option<String> {
    let mut command = std::process::Command::new("/usr/bin/osascript");
    command.args([
        "-e",
        "tell application \"System Events\" to get name of first application process whose frontmost is true",
    ]);
    let output = crate::system::process::run_with_deadline(
        &mut command,
        "frontmost-app osascript",
        operation_deadline(deadline, std::time::Duration::from_secs(2)).ok()?,
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
fn frontmost_app(_deadline: Deadline) -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn reactivate_app(name: &str, deadline: Deadline) {
    let script = format!("tell application {} to activate", applescript_string(name));
    let mut command = std::process::Command::new("/usr/bin/osascript");
    command.arg("-e").arg(script);
    if let Err(e) = crate::system::process::run_with_deadline(
        &mut command,
        "reactivate-app osascript",
        operation_deadline(deadline, std::time::Duration::from_secs(1))
            .unwrap_or_else(|_| std::time::Instant::now()),
    ) {
        tracing::warn!("reactivate_app osascript failed for app {:?}: {e}", name);
    }
}

#[cfg(target_os = "macos")]
fn applescript_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        if ch.is_control() {
            continue;
        }
        if matches!(ch, '\\' | '"') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped.push('"');
    escaped
}

#[cfg(not(target_os = "macos"))]
fn reactivate_app(_name: &str, _deadline: Deadline) {}

#[cfg(target_os = "macos")]
fn nc_pid(deadline: Deadline) -> Result<Option<i32>, AdapterError> {
    let mut command = std::process::Command::new("/usr/bin/pgrep");
    command.arg("-x").arg("NotificationCenter");
    let output = crate::system::process::run_with_deadline(
        &mut command,
        "pgrep NotificationCenter",
        operation_deadline(deadline, std::time::Duration::from_secs(1))?,
    );
    nc_pid_from_output(output)
}

#[cfg(target_os = "macos")]
fn nc_pid_from_output(
    output: Result<std::process::Output, AdapterError>,
) -> Result<Option<i32>, AdapterError> {
    let output = output?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<i32>().ok()))
}

#[cfg(target_os = "macos")]
fn is_nc_open(pid: i32, deadline: Deadline) -> bool {
    use crate::tree::element_for_pid;

    let app = element_for_pid(pid);
    let windows = crate::notifications::read::children_for_attribute(&app, "AXWindows", deadline)
        .unwrap_or_default();
    !windows.is_empty()
}

#[cfg(not(target_os = "macos"))]
fn nc_pid(_deadline: Deadline) -> Result<Option<i32>, AdapterError> {
    Ok(None)
}

#[cfg(not(target_os = "macos"))]
fn is_nc_open(_pid: i32, _deadline: Deadline) -> bool {
    false
}

#[cfg(target_os = "macos")]
fn open_nc(deadline: Deadline) -> Result<(), AdapterError> {
    let script = r#"tell application "System Events" to tell its application process "ControlCenter"
        click (first menu bar item of menu bar 1 whose description is "Clock")
    end tell"#;

    let mut command = std::process::Command::new("/usr/bin/osascript");
    command.arg("-e").arg(script);
    crate::system::process::run_with_deadline(
        &mut command,
        "osascript open-nc",
        operation_deadline(deadline, std::time::Duration::from_secs(2))?,
    )?;
    std::thread::sleep(std::time::Duration::from_millis(500));
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn open_nc(_deadline: Deadline) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("open_nc"))
}

#[cfg(target_os = "macos")]
fn close_nc(deadline: Deadline) -> Result<(), AdapterError> {
    use crate::input::keyboard;

    let combo = KeyCombo {
        key: "escape".into(),
        modifiers: vec![],
    };
    keyboard::synthesize_key(&combo, None, deadline)?;
    std::thread::sleep(std::time::Duration::from_millis(300));
    Ok(())
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::{applescript_string, nc_pid_from_output};
    use agent_desktop_core::AdapterError;

    #[test]
    fn applescript_string_escapes_quotes_and_backslashes() {
        assert_eq!(
            applescript_string(r#"Bad \ "Name""#),
            r#""Bad \\ \"Name\"""#
        );
    }

    #[test]
    fn applescript_string_strips_control_chars() {
        assert_eq!(applescript_string("a\nb"), r#""ab""#);
        assert_eq!(applescript_string("a\tb"), r#""ab""#);
        assert_eq!(applescript_string("a\\\nb"), r#""a\\b""#);
        assert_eq!(applescript_string("a\"b\nc"), r#""a\"bc""#);
    }

    #[test]
    fn nc_pid_preserves_probe_errors() {
        let error = nc_pid_from_output(Err(AdapterError::timeout("pid probe timed out")))
            .expect_err("timeout must not become process-not-found");

        assert_eq!(error.code, agent_desktop_core::ErrorCode::Timeout);
    }
}

#[cfg(not(target_os = "macos"))]
fn close_nc(_deadline: Deadline) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("close_nc"))
}

#[cfg(target_os = "macos")]
fn wait_for_nc_ready(deadline: Deadline) -> Result<i32, AdapterError> {
    let poll = std::time::Duration::from_millis(50);

    loop {
        if let Some(pid) = nc_pid(deadline)? {
            if is_nc_open(pid, deadline) {
                return Ok(pid);
            }
        }
        if deadline.is_expired() {
            return Err(AdapterError::timeout(
                "Notification Center did not open within the operation deadline",
            ));
        }
        std::thread::sleep(poll.min(deadline.remaining()));
    }
}

#[cfg(not(target_os = "macos"))]
fn wait_for_nc_ready(_deadline: Deadline) -> Result<i32, AdapterError> {
    Err(AdapterError::not_supported("wait_for_nc_ready"))
}

fn operation_deadline(
    deadline: Deadline,
    maximum: std::time::Duration,
) -> Result<std::time::Instant, AdapterError> {
    let remaining = deadline.remaining();
    if remaining.is_zero() {
        Err(deadline.timeout_error())
    } else {
        std::time::Instant::now()
            .checked_add(remaining.min(maximum))
            .ok_or_else(|| AdapterError::timeout("Notification subprocess deadline overflowed"))
    }
}
