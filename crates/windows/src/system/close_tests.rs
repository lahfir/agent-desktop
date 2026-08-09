use super::*;
use crate::system::process_identity;
use agent_desktop_core::{DeliveryDisposition, ProcessId};

#[test]
fn protected_process_is_refused_before_any_native_close() {
    let app = AppInfo {
        name: "explorer.exe".into(),
        pid: ProcessId::from(1u32),
        bundle_id: None,
        process_instance: Some("windows-proc-v1:1:1".into()),
    };
    let error = close_app_impl(&app, true, Deadline::after(1_000).expect("deadline"))
        .expect_err("protected");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert!(error.message.contains("protected"));
    assert!(error.suggestion.is_some());
}

#[test]
fn close_requires_a_creation_time_token() {
    let app = AppInfo {
        name: "notepad.exe".into(),
        pid: ProcessId::from(1u32),
        bundle_id: None,
        process_instance: None,
    };
    let error = close_app_impl(&app, false, Deadline::after(1_000).expect("deadline"))
        .expect_err("token required");
    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[test]
fn windowless_alive_error_names_pid_and_stays_not_delivered() {
    let error = AdapterError::new(
        ErrorCode::ActionFailed,
        "Process 42 has no top-level windows to receive WM_CLOSE",
    )
    .with_details(serde_json::json!({ "pid": 42 }))
    .with_suggestion("Retry with --force to terminate pid 42 without WM_CLOSE")
    .with_disposition(DeliverySemantics::not_delivered());
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert!(error.message.contains("42"));
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|text| text.contains("force") && text.contains("42"))
    );
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
}

#[cfg(target_os = "windows")]
fn deadline() -> Deadline {
    Deadline::after(10_000).expect("deadline")
}

#[cfg(target_os = "windows")]
fn app_for_pid(name: &str, pid: ProcessId) -> AppInfo {
    let token = process_identity::token_for_pid(pid)
        .expect("token read")
        .expect("live token");
    AppInfo {
        name: name.into(),
        pid,
        bundle_id: None,
        process_instance: Some(token),
    }
}

#[cfg(target_os = "windows")]
fn process_still_alive(pid: ProcessId, instance: &str) -> bool {
    !super::process_observed_gone(pid, instance).expect("liveness")
}

#[cfg(target_os = "windows")]
fn spawn_windowless_child() -> (std::process::Child, ProcessId, String) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let child = std::process::Command::new("cmd")
        .args(["/C", "ping", "-n", "60", "127.0.0.1", ">", "NUL"])
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .expect("windowless child");
    let pid = ProcessId::from(child.id());
    let started = std::time::Instant::now();
    let token = loop {
        if let Ok(Some(token)) = process_identity::token_for_pid(pid) {
            break token;
        }
        if started.elapsed() > std::time::Duration::from_secs(5) {
            panic!("windowless child never exposed a creation-time token");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    (child, pid, token)
}

#[cfg(target_os = "windows")]
#[test]
fn graceful_hosted_fixture_ok_only_after_independently_observed_gone() {
    crate::tree::fixture::bootstrap();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture");
    let pid = ProcessId::from(fixture.process_id());
    let app = app_for_pid("fixture-host", pid);
    let instance = app.process_instance.clone().expect("token");

    close_app_impl(&app, false, deadline()).expect("graceful close");

    assert!(
        super::process_observed_gone(pid, &instance).expect("gone check"),
        "Ok(()) requires independent exit observation, not API success alone"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn force_terminates_wm_close_ignoring_process_and_verifies_exit() {
    crate::tree::fixture::bootstrap();
    let fixture = crate::tree::fixture::HostedFixture::spawn_swallowing_wm_close()
        .expect("swallowing fixture");
    let pid = ProcessId::from(fixture.process_id());
    let app = app_for_pid("fixture-host", pid);
    let instance = app.process_instance.clone().expect("token");

    close_app_impl(&app, true, deadline()).expect("force close");

    assert!(
        super::process_observed_gone(pid, &instance).expect("gone check"),
        "force must TerminateProcess and verify exit against a WM_CLOSE-swallowing host"
    );
}

/// Invert: if `wait_for_exit` returned `Ok(())` before observing process exit,
/// this timeout assertion would fail (go red).
#[cfg(target_os = "windows")]
#[test]
fn graceful_deadline_timeout_is_delivered_unverified() {
    let _stalled = crate::tree::fixture::StalledFixture::create().expect("stalled");
    let pid = ProcessId::from(std::process::id());
    let app = app_for_pid("test-runner", pid);
    let short = Deadline::after(200).expect("short deadline");

    let error = close_app_impl(&app, false, short).expect_err("timeout");

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    assert!(
        process_still_alive(pid, app.process_instance.as_deref().expect("token")),
        "timeout must not terminate the caller; invert would pass if Ok returned early"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn already_dead_pid_is_benign_ok() {
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "exit", "/B", "0"])
        .spawn()
        .expect("spawn");
    let pid = ProcessId::from(child.id());
    let app = app_for_pid("cmd.exe", pid);
    let _ = child.wait();

    close_app_impl(&app, false, deadline()).expect("already dead is Ok");
}

#[cfg(target_os = "windows")]
#[test]
fn race_windows_gone_because_process_died_is_benign_ok() {
    crate::tree::fixture::bootstrap();
    let mut fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture");
    let pid = ProcessId::from(fixture.process_id());
    let app = app_for_pid("fixture-host", pid);
    fixture.terminate();

    close_app_impl(&app, false, deadline())
        .expect("empty window set after death must follow the handle check, not ACTION_FAILED");
}

#[cfg(target_os = "windows")]
#[test]
fn windowless_alive_graceful_is_action_failed_not_delivered_without_terminate() {
    let (mut child, pid, token) = spawn_windowless_child();
    let app = AppInfo {
        name: "cmd.exe".into(),
        pid,
        bundle_id: None,
        process_instance: Some(token.clone()),
    };
    let windows = super::top_level_windows_for_pid(pid).expect("enumerate");
    assert!(
        windows.is_empty(),
        "CREATE_NO_WINDOW child must present an empty top-level set"
    );

    let error = close_app_impl(&app, false, deadline()).expect_err("windowless alive");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered
    );
    assert!(error.message.contains(&pid.to_string()));
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|text| text.contains("force") && text.contains(&pid.to_string()))
    );
    assert!(
        process_still_alive(pid, &token),
        "graceful windowless must never silent-TerminateProcess"
    );
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(target_os = "windows")]
#[test]
fn mismatched_creation_token_is_benign_ok_without_killing_live_pid() {
    crate::tree::fixture::bootstrap();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture");
    let pid = ProcessId::from(fixture.process_id());
    let live = process_identity::token_for_pid(pid)
        .expect("token")
        .expect("live");
    let app = AppInfo {
        name: "fixture-host".into(),
        pid,
        bundle_id: None,
        process_instance: Some("windows-proc-v1:1:1".into()),
    };

    close_app_impl(&app, true, deadline()).expect("token mismatch is already-gone");

    assert!(
        process_still_alive(pid, &live),
        "exit verification must use pid+token; a mismatched token must not TerminateProcess"
    );
}

#[cfg(target_os = "windows")]
#[test]
fn force_terminate_handle_token_must_match_before_kill() {
    use windows_sys::Win32::Foundation::CloseHandle;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, PROCESS_TERMINATE,
    };

    crate::tree::fixture::bootstrap();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture");
    let pid = ProcessId::from(fixture.process_id());
    let live = process_identity::token_for_pid(pid)
        .expect("token")
        .expect("live");
    let access = PROCESS_TERMINATE | PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE;
    let handle = unsafe { OpenProcess(access, 0, u32::from(pid)) };
    assert!(!handle.is_null());
    assert!(
        super::handle_matches_instance(handle, pid, &live).expect("read"),
        "live handle must match its creation-time token"
    );
    assert!(
        !super::handle_matches_instance(handle, pid, "windows-proc-v1:1:1").expect("read"),
        "mismatched token on the open handle must refuse terminate"
    );
    unsafe { CloseHandle(handle) };
}

#[cfg(target_os = "windows")]
#[test]
fn graceful_wm_close_skips_hwnd_whose_owner_pid_changed() {
    crate::tree::fixture::bootstrap();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture");
    let foreign = crate::tree::fixture::LocalFixture::create().expect("foreign");
    let pid = ProcessId::from(fixture.process_id());
    let token = process_identity::token_for_pid(pid)
        .expect("token")
        .expect("live");
    let delivered = super::post_wm_close_if_still_owned(foreign.handle(), pid, &token)
        .expect("foreign hwnd is skipped, not an error");
    assert!(
        !delivered,
        "a foreign hwnd is reported as not delivered, not as a send"
    );
    assert!(
        process_still_alive(pid, &token),
        "skipping a foreign hwnd must not close the target process"
    );
    let _ = fixture;
}

fn unusable_window() -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, "PostMessageW(WM_CLOSE) failed")
}

/// R2's fan-out rule: one window's refusal must not cancel delivery to the
/// siblings, because the window that owns an app's shutdown may be enumerated
/// after a window that has already torn itself down.
#[test]
fn a_failing_window_does_not_cancel_close_delivery_to_the_others() {
    let mut attempted = Vec::new();
    let result = super::broadcast_close(&[1, 2, 3], Deadline::after(1_000).expect("deadline"), {
        let attempted = &mut attempted;
        move |hwnd| {
            attempted.push(hwnd);
            if hwnd == 1 {
                Err(unusable_window())
            } else {
                Ok(true)
            }
        }
    });

    assert!(
        result.is_ok(),
        "a delivered sibling makes the fan-out a success"
    );
    assert_eq!(
        attempted,
        vec![1, 2, 3],
        "every owned window is attempted even after one fails"
    );
}

/// The honest converse: when nothing accepted the request, the failure is
/// reported rather than swallowed into a silent success.
#[test]
fn a_fan_out_that_delivers_nothing_reports_the_failure() {
    let error = super::broadcast_close(&[1, 2], Deadline::after(1_000).expect("deadline"), |_| {
        Err(unusable_window())
    })
    .expect_err("no window accepted the close");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::NotDelivered,
        "nothing was delivered, so the caller may safely retry"
    );
}

/// A window skipped on ownership grounds is not a failure, so a fan-out of
/// nothing-but-skips succeeds and leaves the verdict to the exit observation.
#[test]
fn skipped_windows_alone_are_not_a_close_failure() {
    super::broadcast_close(&[1, 2], Deadline::after(1_000).expect("deadline"), |_| {
        Ok(false)
    })
    .expect("skips are not failures");
}

/// `259` is `STILL_ACTIVE`, the same value `GetExitCodeProcess` reports for a
/// live process, so a process that legitimately exits with it must still be
/// observed gone. The wait signal is what proves the exit; vetoing on the code
/// would leave a cleanly exited process looking alive for as long as a handle
/// keeps it from being reaped - which the child handle held here does.
#[cfg(target_os = "windows")]
#[test]
fn an_exit_code_of_still_active_is_still_observed_as_gone() {
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "exit", "/B", "259"])
        .spawn()
        .expect("spawn");
    let pid = ProcessId::from(child.id());
    let token = process_identity::token_for_pid(pid)
        .expect("token read")
        .expect("live token");
    child.wait().expect("child exits");

    assert!(
        super::process_observed_gone(pid, &token).expect("liveness"),
        "a process that exited with 259 must not read as still running"
    );
}
