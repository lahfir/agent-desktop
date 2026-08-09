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
