use agent_desktop_core::{
    AdapterError, Deadline, DeliverySemantics, ErrorCode, ProcessId, WindowFilter, WindowInfo,
    launch_options::LaunchOptions, launch_result::LaunchResult,
};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;

use super::app_ops::{ProcessRow, process_snapshot};
use super::launch_path::{
    image_file_name, resolve_executable, validate_app_identifier, validate_launch_options,
};
use super::permissions::ensure_budget;
use super::process_identity;
use super::process_state::hresult_from_win32;
use super::window_ops::list_windows_live;

pub(crate) fn launch_app_impl(
    id: &str,
    options: &LaunchOptions,
    parent_deadline: Deadline,
) -> Result<LaunchResult, AdapterError> {
    validate_app_identifier(id).map_err(before_launch)?;
    validate_launch_options(options).map_err(before_launch)?;
    let deadline = if options.timeout_ms == 0 {
        parent_deadline
    } else {
        parent_deadline.capped(Duration::from_millis(options.timeout_ms))
    };
    ensure_budget(deadline).map_err(before_launch)?;
    let image = image_file_name(id);
    let matches = matching_processes(image).map_err(before_launch)?;
    if options.attach_if_running {
        if matches.len() > 1 {
            return Err(before_launch(ambiguous_apps(&matches)));
        }
        if let Some(row) = matches.into_iter().next() {
            let token = process_identity::token_for_pid(row.pid)
                .map_err(before_launch)?
                .ok_or_else(|| {
                    before_launch(AdapterError::new(
                        ErrorCode::AppUnresponsive,
                        "Running process has no exact process instance token",
                    ))
                })?;
            let window = observe_window(row.pid, &token, options, deadline)?;
            return Ok(launch_result(id, row.pid, token, window));
        }
    } else if let Some(row) = matches.first() {
        return Err(before_launch(already_running_error(row.pid, &matches)));
    }
    let executable = resolve_executable(id).map_err(before_launch)?;
    let (pid, token) = create_process(&executable, options, deadline)?;
    let window = observe_window(pid, &token, options, deadline)?;
    Ok(launch_result(id, pid, token, window))
}

/// The window a launch actually caused, or `None`. A process that presents no
/// window is a fact the caller reads from `LaunchResult.window`, not a failure:
/// a background or windowless process is a legitimate launch outcome, and
/// erroring on it made every such launch spend its whole timeout first.
fn observe_window(
    pid: ProcessId,
    process_instance: &str,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    let mut poll_interval = Duration::from_millis(50);
    loop {
        if let Some(window) = observe_window_once(pid, process_instance, deadline)? {
            return Ok(Some(window));
        }
        if !should_poll_after_first_observation(options.timeout_ms)
            || deadline.remaining().is_zero()
        {
            return Ok(None);
        }
        let remaining = deadline.remaining();
        std::thread::sleep(poll_interval.min(remaining));
        poll_interval = (poll_interval * 3 / 2).min(Duration::from_millis(250));
    }
}

fn launch_result(
    id: &str,
    pid: ProcessId,
    process_instance: String,
    window: Option<WindowInfo>,
) -> LaunchResult {
    LaunchResult {
        app: window
            .as_ref()
            .map(|window| window.app.clone())
            .unwrap_or_else(|| image_file_name(id).to_string()),
        pid,
        process_instance: Some(process_instance),
        window,
        cdp: None,
        renderer: None,
        suggestion: None,
    }
}

/// One window observation, tolerant of the inventory's mid-walk identity race.
///
/// `list_windows_live` now absorbs that race itself (the shared listing-retry
/// budget every caller on this path shares), retrying internally before ever
/// returning `WINDOW_NOT_FOUND` — so this function no longer re-reads the
/// inventory itself. A launch is exactly the moment that race is most
/// reachable, since a process was just created or terminated, but a race that
/// still survives the shared budget, and a budget that runs out entirely
/// (`TIMEOUT`), both say the desktop could not be read coherently in time —
/// not that this launch failed — so both are reported as no window observed.
/// Every other failure still propagates: a target whose process instance
/// changed underneath the launch is a real answer, not a race.
fn observe_window_once(
    pid: ProcessId,
    process_instance: &str,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    match exact_window(pid, process_instance, deadline) {
        Ok(window) => Ok(window),
        Err(error) if is_unobservable_within_budget(&error) => Ok(None),
        Err(error) => Err(after_launch(error)),
    }
}

fn is_unobservable_within_budget(error: &AdapterError) -> bool {
    matches!(error.code, ErrorCode::WindowNotFound | ErrorCode::Timeout)
}

fn exact_window(
    pid: ProcessId,
    process_instance: &str,
    deadline: Deadline,
) -> Result<Option<WindowInfo>, AdapterError> {
    let windows = list_windows_live(
        &WindowFilter {
            focused_only: false,
            app: None,
        },
        deadline,
    )?;
    let Some(window) = windows.into_iter().find(|window| window.pid == pid) else {
        return Ok(None);
    };
    if window.process_instance.as_deref() != Some(process_instance) {
        return Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "Application process instance changed while waiting for its window",
        )
        .with_details(serde_json::json!({ "pid": pid, "complete": false })));
    }
    Ok(Some(window))
}

fn matching_processes(image: &str) -> Result<Vec<ProcessRow>, AdapterError> {
    Ok(process_snapshot()?
        .into_iter()
        .filter(|row| row.name.eq_ignore_ascii_case(image))
        .collect())
}

#[cfg(target_os = "windows")]
fn create_process(
    executable: &Path,
    options: &LaunchOptions,
    deadline: Deadline,
) -> Result<(ProcessId, String), AdapterError> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        CREATE_UNICODE_ENVIRONMENT, CreateProcessW, PROCESS_INFORMATION, STARTUPINFOW,
    };

    ensure_budget(deadline).map_err(before_launch)?;
    let app_wide = to_wide(executable).map_err(before_launch)?;
    let command_line = command_line_for(executable, &options.args).map_err(before_launch)?;
    let mut command_wide = to_wide_str(&command_line).map_err(before_launch)?;
    let cwd_wide = match &options.cwd {
        Some(cwd) => Some(to_wide(cwd).map_err(before_launch)?),
        None => None,
    };
    let env_block = if options.env.is_empty() {
        None
    } else {
        Some(environment_block(&options.env).map_err(before_launch)?)
    };
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut information = PROCESS_INFORMATION::default();
    let flags = if env_block.is_some() {
        CREATE_UNICODE_ENVIRONMENT
    } else {
        0
    };
    let env_ptr = env_block
        .as_ref()
        .map(|block| block.as_ptr().cast())
        .unwrap_or(std::ptr::null());
    let cwd_ptr = cwd_wide
        .as_ref()
        .map(|wide| wide.as_ptr())
        .unwrap_or(std::ptr::null());
    ensure_budget(deadline).map_err(before_launch)?;
    let ok = unsafe {
        CreateProcessW(
            app_wide.as_ptr(),
            command_wide.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            flags,
            env_ptr,
            cwd_ptr,
            &startup,
            &mut information,
        )
    };
    if ok == 0 {
        return Err(before_launch(win32_last_error(
            "CreateProcessW failed to start the application",
        )));
    }
    let pid = ProcessId::from(information.dwProcessId);
    unsafe {
        CloseHandle(information.hThread);
        CloseHandle(information.hProcess);
    }
    let token = process_identity::token_for_pid(pid)
        .map_err(after_launch)?
        .ok_or_else(|| {
            after_launch(
                AdapterError::new(
                    ErrorCode::AppUnresponsive,
                    "Created process has no exact process instance token",
                )
                .with_details(serde_json::json!({ "pid": pid, "complete": false })),
            )
        })?;
    Ok((pid, token))
}

#[cfg(not(target_os = "windows"))]
fn create_process(
    _executable: &Path,
    _options: &LaunchOptions,
    _deadline: Deadline,
) -> Result<(ProcessId, String), AdapterError> {
    Err(AdapterError::not_supported("launch_app"))
}

fn command_line_for(executable: &Path, args: &[String]) -> Result<String, AdapterError> {
    let mut line = quote_arg(&executable.to_string_lossy());
    for arg in args {
        line.push(' ');
        line.push_str(&quote_arg(arg));
    }
    Ok(line)
}

/// Encodes one argument for the command line the child parses back apart.
///
/// The trailing backslash run is the case that silently corrupts: backslashes
/// are literal except immediately before a quote, so a value ending in one
/// would escape the closing quote this appends, leaving the quoted region open
/// and swallowing every later argument into it. That run is therefore doubled.
/// An embedded quote stays doubled rather than backslash-escaped: that is a
/// valid encoding for the consecutive-quote rule and the form `cmd` accepts,
/// which backslash-escaping is not.
fn quote_arg(value: &str) -> String {
    if !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c == '"') {
        return value.to_string();
    }
    let mut out = String::from("\"");
    for ch in value.chars() {
        if ch == '"' {
            out.push('"');
        }
        out.push(ch);
    }
    let trailing = value.len() - value.trim_end_matches('\\').len();
    for _ in 0..trailing {
        out.push('\\');
    }
    out.push('"');
    out
}

/// Windows environment variable names are case-insensitive: a caller
/// overriding `Path` while the parent process carries `PATH` must replace
/// that entry outright, or the child inherits both and resolves whichever
/// one happens to sort first. Keys are folded to ASCII uppercase only to
/// detect that collision; the spelling that lands in the block is always
/// the caller's own for an override, or the inherited spelling otherwise.
fn environment_block(overrides: &BTreeMap<String, String>) -> Result<Vec<u16>, AdapterError> {
    let mut merged: BTreeMap<String, (String, String)> = std::env::vars()
        .map(|(key, value)| (key.to_ascii_uppercase(), (key, value)))
        .collect();
    for (key, value) in overrides {
        merged.insert(key.to_ascii_uppercase(), (key.clone(), value.clone()));
    }
    let mut block = Vec::new();
    for (key, value) in merged.into_values() {
        if key.contains('=') || key.contains('\0') || value.contains('\0') {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                "Launch environment entries must not contain NUL or '=' in the key",
            ));
        }
        for unit in format!("{key}={value}").encode_utf16() {
            block.push(unit);
        }
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

#[cfg(target_os = "windows")]
fn to_wide(path: &Path) -> Result<Vec<u16>, AdapterError> {
    use std::os::windows::ffi::OsStrExt;
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch path contains an interior NUL",
        ));
    }
    wide.push(0);
    Ok(wide)
}

#[cfg(target_os = "windows")]
fn to_wide_str(value: &str) -> Result<Vec<u16>, AdapterError> {
    if value.contains('\0') {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch command line contains an interior NUL",
        ));
    }
    let mut wide: Vec<u16> = value.encode_utf16().collect();
    wide.push(0);
    Ok(wide)
}

#[cfg(target_os = "windows")]
fn win32_last_error(message: &str) -> AdapterError {
    let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    adapter_error_from_win32(error, message)
}

fn adapter_error_from_win32(error: u32, message: &str) -> AdapterError {
    let hresult = hresult_from_win32(error);
    let record = super::hresult::hresult_record(hresult);
    let mut err = AdapterError::new(record.code, message)
        .with_platform_detail(super::hresult::com_hresult_detail(hresult));
    if let Some(suggestion) = record.suggestion {
        err = err.with_suggestion(suggestion);
    }
    err
}

fn already_running_error(pid: ProcessId, matches: &[ProcessRow]) -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, "Application is already running")
        .with_details(serde_json::json!({
            "pid": pid,
            "candidate_pids": matches.iter().map(|row| row.pid).collect::<Vec<_>>(),
        }))
        .with_suggestion("Omit --no-attach to attach to the running instance, or close it first.")
}

fn ambiguous_apps(matches: &[ProcessRow]) -> AdapterError {
    AdapterError::ambiguous_target("More than one application instance matches the launch target")
        .with_details(serde_json::json!({
            "candidate_pids": matches.iter().map(|row| row.pid).collect::<Vec<_>>(),
        }))
}

fn before_launch(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}

fn after_launch(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

fn should_poll_after_first_observation(timeout_ms: u64) -> bool {
    timeout_ms > 0
}

#[cfg(test)]
#[path = "launch_tests.rs"]
mod tests;
