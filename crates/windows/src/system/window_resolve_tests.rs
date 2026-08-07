use super::*;
use crate::system::window_ops::list_windows_live;
use agent_desktop_core::WindowFilter;

#[cfg(target_os = "windows")]
#[test]
fn resolve_window_strict_reconfirms_a_listed_fixture_window() {
    crate::tree::fixture::ensure_test_apartment();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture host starts");
    let windows = list_windows_live(&WindowFilter::default()).expect("list");
    let expected = windows
        .into_iter()
        .find(|window| window.pid == agent_desktop_core::ProcessId::from(fixture.process_id()))
        .expect("fixture window listed");
    let resolved =
        resolve_window_strict(&expected, Deadline::after(5_000).unwrap()).expect("resolve");
    assert_eq!(resolved.id, expected.id);
    assert_eq!(resolved.pid, expected.pid);
    assert_eq!(resolved.process_instance, expected.process_instance);
}

#[cfg(target_os = "windows")]
#[test]
fn resolve_window_strict_rejects_a_destroyed_handle() {
    let win = WindowInfo {
        id: "w-1".into(),
        title: "gone".into(),
        app: "none.exe".into(),
        pid: agent_desktop_core::ProcessId::from(1u32),
        process_instance: Some("windows-proc-v1:0:0".into()),
        bounds: None,
        state: WindowState::default(),
    };
    let err = resolve_window_strict(&win, Deadline::after(1_000).unwrap()).unwrap_err();
    assert_eq!(err.code, ErrorCode::WindowNotFound);
}

#[cfg(target_os = "windows")]
#[test]
fn focus_window_refuses_a_destroyed_handle_before_any_window_write() {
    let win = WindowInfo {
        id: "w-1".into(),
        title: "gone".into(),
        app: "none.exe".into(),
        pid: agent_desktop_core::ProcessId::from(1u32),
        process_instance: Some("windows-proc-v1:0:0".into()),
        bounds: None,
        state: WindowState::default(),
    };
    let lease = InteractionLease::guarded(Deadline::after(1_000).unwrap(), ()).expect("lease");
    let err = focus_window(&win, &lease).unwrap_err();
    assert_eq!(err.code, ErrorCode::WindowNotFound);
}

/// The success contract, pinned without asserting a machine-specific
/// outcome: whether a lane can take the foreground at all depends on the
/// desktop it runs on, so this asserts the implication rather than the
/// result. `Ok` must mean the foreground window is the target **and** is
/// still owned by the expected process; a lane that cannot focus must say
/// so as a not-delivered failure. The forbidden state is `Ok` while some
/// other process owns the foreground.
#[cfg(target_os = "windows")]
#[test]
fn focus_window_reports_ok_only_when_the_expected_process_owns_the_foreground() {
    crate::tree::fixture::ensure_test_apartment();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture host starts");
    let windows = list_windows_live(&WindowFilter::default()).expect("list");
    let expected = windows
        .into_iter()
        .find(|window| window.pid == agent_desktop_core::ProcessId::from(fixture.process_id()))
        .expect("fixture window listed");
    let lease = InteractionLease::guarded(Deadline::after(5_000).unwrap(), ()).expect("lease");
    match focus_window(&expected, &lease) {
        Ok(()) => {
            let handle = parse_handle(&expected.id);
            assert!(
                is_owned_foreground(handle, expected.pid),
                "focus_window returned Ok while the foreground was not the owned target"
            );
        }
        Err(error) => {
            assert_eq!(error.code, ErrorCode::ActionFailed);
            assert_eq!(
                error
                    .details
                    .as_ref()
                    .and_then(|details| details.get("physical_delivery_started")),
                Some(&serde_json::Value::Bool(false)),
                "a focus failure must report that no input was delivered"
            );
        }
    }
}

#[cfg(target_os = "windows")]
#[test]
fn focus_window_refuses_an_expired_lease_before_any_window_write() {
    crate::tree::fixture::ensure_test_apartment();
    let fixture = crate::tree::fixture::HostedFixture::spawn().expect("fixture host starts");
    let windows = list_windows_live(&WindowFilter::default()).expect("list");
    let expected = windows
        .into_iter()
        .find(|window| window.pid == agent_desktop_core::ProcessId::from(fixture.process_id()))
        .expect("fixture window listed");
    let lease = InteractionLease::guarded(Deadline::after(1).unwrap(), ()).expect("lease");
    std::thread::sleep(std::time::Duration::from_millis(5));
    let err = focus_window(&expected, &lease).unwrap_err();
    assert_eq!(err.code, ErrorCode::Timeout);
}

#[cfg(target_os = "windows")]
#[test]
fn resolve_window_strict_refuses_an_expired_deadline() {
    let win = WindowInfo {
        id: "w-1".into(),
        title: "gone".into(),
        app: "none.exe".into(),
        pid: agent_desktop_core::ProcessId::from(1u32),
        process_instance: Some("windows-proc-v1:0:0".into()),
        bounds: None,
        state: WindowState::default(),
    };
    let deadline = Deadline::after(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(5));
    let err = resolve_window_strict(&win, deadline).unwrap_err();
    assert_eq!(err.code, ErrorCode::Timeout);
}

#[cfg(target_os = "windows")]
#[test]
fn focus_window_requires_a_process_instance_token() {
    let win = WindowInfo {
        id: "w-1".into(),
        title: "gone".into(),
        app: "none.exe".into(),
        pid: agent_desktop_core::ProcessId::from(1u32),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    };
    let lease = InteractionLease::guarded(Deadline::after(1_000).unwrap(), ()).expect("lease");
    let err = focus_window(&win, &lease).unwrap_err();
    assert_eq!(err.code, ErrorCode::StaleRef);
}
