use super::super::*;
use super::{
    KillOnDrop, copy_probe_binary, copy_system_exe, deadline, full_image_path, matching_pids,
    probe_launch_options, scratch_dir, start_named_windowless, start_windowed_probe,
};
use agent_desktop_core::{Deadline, DeliveryDisposition, ErrorCode, ProcessId};
use std::sync::Mutex;

static NOTEPAD_LAUNCH_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn scratch_launch_returns_window_for_created_process() {
    let _lock = NOTEPAD_LAUNCH_LOCK.lock().expect("notepad lock");
    crate::tree::fixture::bootstrap();
    let dir = scratch_dir("notepad-scratch");
    let probe = copy_probe_binary(&dir, "launchprobe_host_scratch.exe");
    let image = "launchprobe_host_scratch.exe";
    let window = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &probe_launch_options(false, 8_000),
        deadline(),
    )
    .expect("launch notepad probe");
    let _guard = KillOnDrop(window.pid);
    assert!(
        matching_pids(image).contains(&window.pid),
        "returned window pid must belong to the probe notepad process"
    );
    assert!(window.process_instance.is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn attach_true_reuses_single_running_instance() {
    let _lock = NOTEPAD_LAUNCH_LOCK.lock().expect("notepad lock");
    crate::tree::fixture::bootstrap();
    let dir = scratch_dir("notepad-attach");
    let probe = copy_probe_binary(&dir, "launchprobe_host_attach.exe");
    let image = "launchprobe_host_attach.exe";
    let _first = start_windowed_probe(&probe);
    let before = matching_pids(image);
    assert_eq!(
        before.len(),
        1,
        "fixture must leave exactly one probe notepad"
    );
    let window = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &probe_launch_options(true, 8_000),
        deadline(),
    )
    .expect("attach");
    assert_eq!(window.pid, before[0]);
    assert_eq!(matching_pids(image).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn attach_false_fails_naming_running_pid() {
    let _lock = NOTEPAD_LAUNCH_LOCK.lock().expect("notepad lock");
    crate::tree::fixture::bootstrap();
    let dir = scratch_dir("notepad-noattach");
    let probe = copy_probe_binary(&dir, "launchprobe_host_noattach.exe");
    let image = "launchprobe_host_noattach.exe";
    let _running = start_windowed_probe(&probe);
    let running_pid = matching_pids(image)
        .into_iter()
        .next()
        .expect("probe notepad running");
    let error = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            attach_if_running: false,
            ..Default::default()
        },
        deadline(),
    )
    .expect_err("no-attach must fail");
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    let details = error.details.expect("details");
    assert_eq!(details["pid"], u32::from(running_pid));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn attach_true_windowless_does_not_launch_second_process() {
    let dir = scratch_dir("windowless");
    let probe = copy_system_exe("ping.exe", &dir, "launchprobe_ping.exe");
    let image = "launchprobe_ping.exe";
    let _ping = start_named_windowless(&probe, &["-n", "60", "127.0.0.1"]);
    assert_eq!(matching_pids(image).len(), 1);
    let error = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            attach_if_running: true,
            timeout_ms: 300,
            ..Default::default()
        },
        Deadline::after(1_000).expect("deadline"),
    )
    .expect_err("windowless attach times out waiting for a window");
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    assert_eq!(matching_pids(image).len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn two_matches_are_ambiguous_before_launch() {
    let _lock = NOTEPAD_LAUNCH_LOCK.lock().expect("notepad lock");
    crate::tree::fixture::bootstrap();
    let dir = scratch_dir("notepad-ambiguous");
    let probe = copy_probe_binary(&dir, "launchprobe_host_ambiguous.exe");
    let image = "launchprobe_host_ambiguous.exe";
    let _one = start_windowed_probe(&probe);
    let _two = start_windowed_probe(&probe);
    let before = matching_pids(image);
    assert!(before.len() >= 2);
    let error = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            attach_if_running: true,
            ..Default::default()
        },
        deadline(),
    )
    .expect_err("ambiguous");
    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert_eq!(matching_pids(image).len(), before.len());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cwd_is_honored_for_create_process() {
    let temp = scratch_dir("cwd");
    let probe = copy_system_exe("cmd.exe", &temp, "launchprobe_cwd.exe");
    let marker = temp.join("cwd.txt");
    let options = LaunchOptions {
        args: vec!["/C".into(), format!("cd > \"{}\"", marker.display())],
        cwd: Some(temp.clone()),
        attach_if_running: false,
        timeout_ms: 0,
        ..Default::default()
    };
    let error = launch_app_impl(probe.to_str().expect("utf8 path"), &options, deadline())
        .expect_err("probe is windowless");
    if let Some(pid) = error
        .details
        .as_ref()
        .and_then(|details| details.get("pid"))
        .and_then(|value| value.as_u64())
    {
        let _guard = KillOnDrop(ProcessId::from(pid as u32));
    }
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    let written = poll_read_marker(&marker, Deadline::after(5_000).expect("deadline"));
    let normalized_written = written.trim().trim_end_matches('\\').to_ascii_lowercase();
    let normalized_temp = temp
        .to_string_lossy()
        .trim_end_matches('\\')
        .to_ascii_lowercase();
    assert_eq!(normalized_written, normalized_temp);
    let _ = std::fs::remove_dir_all(&temp);
}

/// The marker is written by a child shell redirecting `cd` output, a step
/// that lands strictly after `CreateProcessW` returns to this test. Reading
/// it once, immediately, races that write; polling under a deadline lets the
/// child finish without turning a slow scheduler into a false failure.
fn poll_read_marker(marker: &std::path::Path, deadline: Deadline) -> String {
    loop {
        if let Ok(contents) = std::fs::read_to_string(marker) {
            if !contents.trim().is_empty() {
                return contents;
            }
        }
        if deadline.remaining().is_zero() {
            return std::fs::read_to_string(marker).expect("cwd marker");
        }
        std::thread::sleep(std::time::Duration::from_millis(50).min(deadline.remaining()));
    }
}

#[test]
fn timeout_zero_checks_once_for_process_without_window() {
    let dir = scratch_dir("timeout0");
    let probe = copy_system_exe("ping.exe", &dir, "launchprobe_timeout.exe");
    let error = launch_app_impl(
        probe.to_str().expect("utf8 path"),
        &LaunchOptions {
            args: vec!["-n".into(), "30".into(), "127.0.0.1".into()],
            attach_if_running: false,
            timeout_ms: 0,
            ..Default::default()
        },
        deadline(),
    )
    .expect_err("no window");
    if let Some(pid) = error
        .details
        .as_ref()
        .and_then(|details| details.get("pid"))
        .and_then(|value| value.as_u64())
    {
        let _guard = KillOnDrop(ProcessId::from(pid as u32));
    }
    assert_eq!(error.code, ErrorCode::WindowNotFound);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The hijack scenario itself needs the literal bare name `notepad.exe`,
/// since it is bare-name resolution against the real system directories that
/// is under test; a uniquely-named probe copy would not exercise it. What it
/// does not need is to hold the process-wide current directory across the
/// launch that follows. Resolution is captured once, under the planted cwd,
/// and the cwd is restored immediately after — before the up-to-8-second
/// window poll runs, and before the already-resolved absolute path is handed
/// to the launch so nothing downstream depends on cwd being anything at all.
#[test]
fn bare_name_never_chooses_planted_binary_in_current_directory() {
    let _lock = NOTEPAD_LAUNCH_LOCK.lock().expect("notepad lock");
    crate::tree::fixture::bootstrap();
    let system_notepad = resolve_executable("notepad.exe").expect("system notepad");
    let plant_dir = scratch_dir("hijack");
    let planted = plant_dir.join("notepad.exe");
    std::fs::write(&planted, b"not a real pe").expect("plant decoy");
    let previous = std::env::current_dir().expect("cwd");
    std::env::set_current_dir(&plant_dir).expect("enter plant dir");
    let resolved = resolve_executable("notepad.exe");
    let _ = std::env::set_current_dir(&previous);
    let resolved = resolved.expect("resolve ignores cwd");
    assert_ne!(
        resolved.canonicalize().ok(),
        planted.canonicalize().ok(),
        "resolver must not select the planted cwd binary"
    );
    assert_eq!(
        resolved.canonicalize().ok(),
        system_notepad.canonicalize().ok()
    );
    let window = launch_app_impl(
        resolved.to_str().expect("utf8 path"),
        &probe_launch_options(false, 8_000),
        deadline(),
    )
    .expect("launch system notepad");
    let _guard = KillOnDrop(window.pid);
    let image = full_image_path(window.pid).expect("image path");
    let image_canon = image.canonicalize().unwrap_or(image);
    let planted_canon = planted.canonicalize().unwrap_or(planted.clone());
    assert_ne!(image_canon, planted_canon);
    assert_eq!(
        image_canon
            .file_name()
            .map(|name| name.to_ascii_lowercase()),
        Some(std::ffi::OsString::from("notepad.exe"))
    );
    let _ = std::fs::remove_dir_all(&plant_dir);
}
