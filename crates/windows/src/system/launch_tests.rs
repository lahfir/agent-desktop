use super::*;
use agent_desktop_core::{Deadline, DeliveryDisposition, ErrorCode};

#[cfg(target_os = "windows")]
use agent_desktop_core::ProcessId;
#[cfg(target_os = "windows")]
use std::path::{Path, PathBuf};

#[cfg(target_os = "windows")]
#[path = "launch_live_tests.rs"]
mod live;

#[test]
fn zero_wait_never_polls_after_first_observation() {
    assert!(!should_poll_after_first_observation(0));
    assert!(should_poll_after_first_observation(1));
}

#[test]
fn launch_options_enforce_bounded_entry_counts() {
    let options = LaunchOptions {
        args: (0..=256).map(|index| index.to_string()).collect(),
        ..Default::default()
    };
    let error = validate_launch_options(&options).expect_err("too many args");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

#[test]
fn launch_options_enforce_a_bounded_text_budget() {
    let options = LaunchOptions {
        args: vec!["x".repeat(1024 * 1024 + 1)],
        ..Default::default()
    };
    let error = validate_launch_options(&options).expect_err("payload too large");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
}

#[test]
fn invalid_identifier_is_not_delivered() {
    let error = launch_app_impl(
        r"sub\app.exe",
        &LaunchOptions::default(),
        Deadline::after(1_000).expect("deadline"),
    )
    .expect_err("relative id");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn elevation_required_maps_through_hresult_from_win32() {
    let hresult = hresult_from_win32(740);
    assert_eq!(hresult, 0x8007_02E4_u32 as i32);
    let error = adapter_error_from_win32(740, "CreateProcessW failed to start the application");
    assert!(
        error
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.to_ascii_uppercase().contains("800702E4")),
        "platform_detail should carry HRESULT_FROM_WIN32(740): {:?}",
        error.platform_detail
    );
}

#[cfg(target_os = "windows")]
fn deadline() -> Deadline {
    Deadline::after(10_000).expect("deadline")
}

#[cfg(target_os = "windows")]
fn matching_pids(image: &str) -> Vec<ProcessId> {
    matching_processes(image)
        .expect("snapshot")
        .into_iter()
        .map(|row| row.pid)
        .collect()
}

#[cfg(target_os = "windows")]
fn terminate_pid(pid: ProcessId) {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_TERMINATE, TerminateProcess};
    let handle = unsafe { OpenProcess(PROCESS_TERMINATE, 0, u32::from(pid)) };
    if !handle.is_null() {
        unsafe {
            TerminateProcess(handle, 1);
            CloseHandle(handle);
        }
    }
}

#[cfg(target_os = "windows")]
struct KillOnDrop(ProcessId);

#[cfg(target_os = "windows")]
impl Drop for KillOnDrop {
    fn drop(&mut self) {
        terminate_pid(self.0);
    }
}

#[cfg(target_os = "windows")]
fn scratch_dir(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "agent-desktop-launch-{label}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[cfg(target_os = "windows")]
fn copy_system_exe(name: &str, dest_dir: &Path, alias: &str) -> PathBuf {
    let source = resolve_executable(name).expect("system executable");
    let dest = dest_dir.join(alias);
    std::fs::copy(&source, &dest).expect("copy executable");
    dest
}

#[cfg(target_os = "windows")]
fn start_notepad() -> KillOnDrop {
    let path = resolve_executable("notepad.exe").expect("system notepad");
    let child = std::process::Command::new(&path)
        .spawn()
        .expect("start notepad");
    let pid = ProcessId::from(child.id());
    std::mem::forget(child);
    let started = std::time::Instant::now();
    while started.elapsed() < std::time::Duration::from_secs(5) {
        let token = match process_identity::token_for_pid(pid) {
            Ok(Some(token)) => token,
            _ => {
                std::thread::sleep(std::time::Duration::from_millis(50));
                continue;
            }
        };
        if exact_window(pid, &token, Deadline::after(200).expect("short"))
            .ok()
            .flatten()
            .is_some()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    KillOnDrop(pid)
}

#[cfg(target_os = "windows")]
fn start_named_windowless(exe: &Path, args: &[&str]) -> KillOnDrop {
    let child = std::process::Command::new(exe)
        .args(args)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("start windowless");
    let pid = ProcessId::from(child.id());
    std::mem::forget(child);
    KillOnDrop(pid)
}

#[cfg(target_os = "windows")]
fn full_image_path(pid: ProcessId) -> Option<PathBuf> {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, u32::from(pid)) };
    if handle.is_null() {
        return None;
    }
    let mut buffer = [0u16; 512];
    let mut size = buffer.len() as u32;
    let ok = unsafe { QueryFullProcessImageNameW(handle, 0, buffer.as_mut_ptr(), &mut size) };
    unsafe { CloseHandle(handle) };
    if ok == 0 || size == 0 {
        return None;
    }
    Some(PathBuf::from(String::from_utf16_lossy(
        &buffer[..size as usize],
    )))
}

#[cfg(target_os = "windows")]
fn clear_notepads() {
    for pid in matching_pids("notepad.exe") {
        terminate_pid(pid);
    }
    std::thread::sleep(std::time::Duration::from_millis(200));
}

/// A value ending in a backslash would otherwise escape the closing quote,
/// leaving the quoted region open so every later argument is swallowed into
/// it. The run is doubled; backslashes elsewhere stay literal.
#[test]
fn a_trailing_backslash_cannot_escape_the_closing_quote() {
    assert_eq!(
        super::quote_arg("C:\\Program Files\\"),
        "\"C:\\Program Files\\\\\""
    );
    assert_eq!(
        super::quote_arg("C:\\dir\\\\"),
        "C:\\dir\\\\",
        "an unquoted value has no closing quote for a backslash to escape"
    );
    assert_eq!(
        super::quote_arg("no trailing\\slash"),
        "\"no trailing\\slash\""
    );
    assert_eq!(super::quote_arg("plain"), "plain");
}
