use agent_desktop_core::{
    adapter::WindowFilter,
    error::{AdapterError, ErrorCode},
    launch_options::LaunchOptions,
    node::WindowInfo,
};

#[cfg(target_os = "macos")]
pub fn launch_app_with_options_impl(
    id: &str,
    options: &LaunchOptions,
    timeout_ms: u64,
) -> Result<WindowInfo, AdapterError> {
    use crate::system::window_list::list_windows_impl;
    use std::process::Command;
    use std::time::{Duration, Instant};

    const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

    let filter = WindowFilter {
        focused_only: false,
        app: Some(id.to_string()),
    };
    if let Ok(wins) = list_windows_impl(&filter) {
        if let Some(win) = wins.into_iter().next() {
            if options.attach {
                return Ok(win);
            }
            return Err(launch_conflict_error(id, win.pid));
        }
    }

    let mut command = Command::new("/usr/bin/open");
    command.args(open_argv(id, &options.args));
    if let Some(cwd) = &options.cwd {
        command.current_dir(cwd);
    }
    for (key, value) in &options.env {
        command.env(key, value);
    }
    crate::system::process::run_with_timeout(&mut command, "open", OPEN_TIMEOUT)?;

    if !options.attach {
        return Ok(WindowInfo {
            id: String::new(),
            title: String::new(),
            app: id.to_string(),
            pid: crate::system::app_list::pid_for_app_name(id).unwrap_or(0),
            bounds: None,
            is_focused: false,
        });
    }

    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut poll_interval = Duration::from_millis(100);
    let max_interval = Duration::from_millis(500);
    loop {
        std::thread::sleep(poll_interval);
        let filter = WindowFilter {
            focused_only: false,
            app: Some(id.to_string()),
        };
        if let Ok(wins) = list_windows_impl(&filter) {
            if let Some(win) = wins.into_iter().next() {
                return Ok(win);
            }
        }
        if start.elapsed() > timeout {
            break;
        }
        poll_interval = (poll_interval * 3 / 2).min(max_interval);
    }
    Err(launch_no_window_error(id, timeout_ms))
}

#[cfg(not(target_os = "macos"))]
pub fn launch_app_with_options_impl(
    _id: &str,
    _options: &LaunchOptions,
    _timeout_ms: u64,
) -> Result<WindowInfo, AdapterError> {
    Err(AdapterError::not_supported("launch_app_with_options"))
}

#[cfg(target_os = "macos")]
pub fn launch_app_impl(id: &str, timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
    tracing::debug!("system: launch app={id:?} timeout={timeout_ms}ms");
    use crate::system::window_list::list_windows_impl;
    use std::process::Command;
    use std::time::{Duration, Instant};

    const OPEN_TIMEOUT: Duration = Duration::from_secs(5);

    if id.contains("..") || id.starts_with('/') {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("Invalid app identifier: '{id}'"),
        )
        .with_suggestion("Use an app name like 'Safari' or bundle ID like 'com.apple.Safari'."));
    }

    let filter = WindowFilter {
        focused_only: false,
        app: Some(id.to_string()),
    };
    if let Ok(wins) = list_windows_impl(&filter) {
        if let Some(win) = wins.into_iter().next() {
            return Ok(win);
        }
    }

    let mut command = Command::new("/usr/bin/open");
    command.args(open_app_args(id));
    crate::system::process::run_with_timeout(&mut command, "open", OPEN_TIMEOUT)?;

    let start = Instant::now();
    let timeout = Duration::from_millis(timeout_ms);
    let mut poll_interval = Duration::from_millis(100);
    let max_interval = Duration::from_millis(500);

    loop {
        std::thread::sleep(poll_interval);
        let filter = WindowFilter {
            focused_only: false,
            app: Some(id.to_string()),
        };
        if let Ok(wins) = list_windows_impl(&filter) {
            if let Some(win) = wins.into_iter().next() {
                return Ok(win);
            }
        }
        if start.elapsed() > timeout {
            break;
        }
        poll_interval = (poll_interval * 3 / 2).min(max_interval);
    }

    Err(launch_no_window_error(id, timeout_ms))
}

#[cfg(not(target_os = "macos"))]
pub fn launch_app_impl(_id: &str, _timeout_ms: u64) -> Result<WindowInfo, AdapterError> {
    Err(AdapterError::not_supported("launch_app"))
}

#[cfg(target_os = "macos")]
fn open_app_args(id: &str) -> [&str; 3] {
    ["-g", "-a", id]
}

/// Assembles the `open` argv for a launch with optional app-args. `--args`
/// is emitted at most once, immediately before all of `args`: `open` treats
/// everything after the first `--args` as literal argv for the launched
/// app, so repeating the flag per element (the prior bug) handed the app a
/// stray `--args` token instead of its second argument.
#[cfg(target_os = "macos")]
fn open_argv(id: &str, args: &[String]) -> Vec<String> {
    let mut argv: Vec<String> = open_app_args(id).into_iter().map(String::from).collect();
    if !args.is_empty() {
        argv.push("--args".to_string());
        argv.extend_from_slice(args);
    }
    argv
}

/// The `--no-attach` conflict path: the app already owns a window, so a
/// caller that explicitly asked not to attach gets a structured refusal
/// instead of a silent re-attach. `pid` is OS-assigned and safe to name
/// directly; `id` is open-ended caller input, so it stays out of `message`
/// and travels only in `details.app_name`, which redacts on trace export.
#[cfg(target_os = "macos")]
fn launch_conflict_error(id: &str, pid: i32) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionFailed,
        format!("App is already running as pid {pid}; refusing to launch again with --no-attach"),
    )
    .with_details(serde_json::json!({ "app_name": id, "pid": pid }))
    .with_suggestion("Close the running instance first, or omit --no-attach to attach to it")
}

#[cfg(target_os = "macos")]
fn launch_no_window_error(id: &str, timeout_ms: u64) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppNotFound,
        format!("Launched app but no window appeared within {timeout_ms} ms"),
    )
    .with_details(serde_json::json!({ "app_name": id }))
    .with_suggestion("The app may take longer to start, or it may not create a visible window")
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
