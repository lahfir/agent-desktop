use crate::input::elevation::activation_elevation_denied;
use crate::system::close::close_app_impl;
use crate::system::launch::launch_app_impl;
use crate::system::window_activate::{budget_exhausted, focus_window};
use crate::system::window_op::window_op_impl;
use agent_desktop_core::launch_options::LaunchOptions;
use agent_desktop_core::process_state::ProcessState;
use agent_desktop_core::{
    AdapterError, AppError, AppInfo, Deadline, DeliverySemantics, ErrorCode, ErrorPayload,
    InteractionLease, ProcessId, WindowInfo, WindowOp, WindowState,
};
use serde_json::Value;

fn error_wire(error: &AdapterError) -> Value {
    serde_json::to_value(ErrorPayload::from_app_error(&AppError::from(error.clone())))
        .expect("ErrorPayload serializes")
}

fn assert_code_wire(error: &AdapterError, code: ErrorCode) {
    assert_eq!(error_wire(error)["code"], code.as_str());
}

fn assert_disposition_wire(error: &AdapterError, expected: DeliverySemantics) {
    let wire = error_wire(error);
    let projected = serde_json::to_value(expected).expect("disposition serializes");
    assert_eq!(wire["disposition"], projected, "disposition wire shape");
}

fn assert_existing_error_code(error: &AdapterError) {
    let wire = error.code.as_str();
    assert_eq!(
        error_wire(error)["code"],
        wire,
        "platform-only failures must serialize an existing ErrorCode"
    );
    assert!(
        matches!(
            error.code,
            ErrorCode::PermDenied
                | ErrorCode::ElementNotFound
                | ErrorCode::AppNotFound
                | ErrorCode::ActionFailed
                | ErrorCode::ActionNotSupported
                | ErrorCode::StaleRef
                | ErrorCode::AmbiguousTarget
                | ErrorCode::WindowNotFound
                | ErrorCode::PlatformNotSupported
                | ErrorCode::Timeout
                | ErrorCode::InvalidArgs
                | ErrorCode::NotificationNotFound
                | ErrorCode::SnapshotNotFound
                | ErrorCode::PolicyDenied
                | ErrorCode::AppUnresponsive
                | ErrorCode::Internal
        ),
        "platform-only failures must use an existing ErrorCode variant"
    );
}

fn assert_class_b_envelope(error: &AdapterError, code: ErrorCode, expected: DeliverySemantics) {
    assert_existing_error_code(error);
    assert_code_wire(error, code);
    assert_disposition_wire(error, expected);
}

fn macos_shared_pair(code: ErrorCode, disposition: DeliverySemantics) -> (ErrorCode, DeliverySemantics) {
    (code, disposition)
}

#[test]
fn shared_timeout_after_close_delivery_matches_macos_pair() {
    let (code, disposition) = macos_shared_pair(
        ErrorCode::Timeout,
        DeliverySemantics::delivered_unverified(),
    );
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (code, disposition);
    }
    #[cfg(target_os = "windows")]
    {
        let _stalled = crate::tree::fixture::StalledFixture::create().expect("stalled");
        let pid = ProcessId::from(std::process::id());
        let token = crate::system::process_identity::token_for_pid(pid)
            .expect("token")
            .expect("live");
        let app = AppInfo {
            name: "test-runner".into(),
            pid,
            bundle_id: None,
            process_instance: Some(token),
        };
        let error = close_app_impl(&app, false, Deadline::after(200).expect("deadline"))
            .expect_err("timeout");
        assert_eq!(error.code, code);
        assert_disposition_wire(&error, disposition);
        assert_eq!(error_wire(&error)["disposition"]["retry"], "unsafe");
    }
}

#[test]
fn shared_stale_ref_before_window_write_matches_macos_pair() {
    let (code, disposition) =
        macos_shared_pair(ErrorCode::StaleRef, DeliverySemantics::not_delivered());
    let win = WindowInfo {
        id: "w-1".into(),
        title: String::new(),
        app: "fixture".into(),
        pid: ProcessId::from(1u32),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    };
    let error = window_op_impl(
        &win,
        WindowOp::Minimize,
        Deadline::after(1_000).expect("deadline"),
    )
    .expect_err("stale");
    assert_eq!(error.code, code);
    assert_disposition_wire(&error, disposition);
    assert_eq!(error_wire(&error)["disposition"]["retry"], "safe");
}

#[test]
fn shared_action_failed_before_delivery_matches_macos_pair() {
    let (code, disposition) = macos_shared_pair(
        ErrorCode::ActionFailed,
        DeliverySemantics::not_delivered(),
    );
    let error = budget_exhausted(false);
    assert_eq!(error.code, code);
    assert_disposition_wire(&error, disposition);
    assert_eq!(error_wire(&error)["disposition"]["retry"], "safe");
}

#[test]
fn shared_ambiguous_target_matches_macos_pair() {
    let (code, disposition) = macos_shared_pair(
        ErrorCode::AmbiguousTarget,
        DeliverySemantics::not_delivered(),
    );
    let error = AdapterError::ambiguous_target(
        "More than one application instance matches the launch target",
    )
    .with_details(serde_json::json!({ "candidate_pids": [10, 11] }));
    assert_eq!(error.code, code);
    assert_disposition_wire(&error, disposition);
    assert_eq!(
        launch_ambiguous_shape(),
        (error.code, error.disposition),
        "launch ambiguous constructor must keep the shared pair"
    );
}

fn launch_ambiguous_shape() -> (ErrorCode, DeliverySemantics) {
    let error = AdapterError::ambiguous_target(
        "More than one application instance matches the launch target",
    );
    (error.code, error.disposition)
}

#[test]
fn shared_perm_denied_before_delivery_matches_macos_pair() {
    let (code, disposition) =
        macos_shared_pair(ErrorCode::PermDenied, DeliverySemantics::not_delivered());
    let error = AdapterError::new(ErrorCode::PermDenied, "Accessibility permission denied")
        .with_disposition(DeliverySemantics::not_delivered());
    assert_eq!(error.code, code);
    assert_disposition_wire(&error, disposition);
    assert_eq!(error_wire(&error)["disposition"]["retry"], "safe");
}

#[test]
fn class_b_windowless_close_escalation_is_action_failed_not_delivered() {
    let error = AdapterError::new(
        ErrorCode::ActionFailed,
        "Process 42 has no top-level windows to receive WM_CLOSE",
    )
    .with_details(serde_json::json!({ "pid": 42 }))
    .with_suggestion("Retry with --force to terminate pid 42 without WM_CLOSE")
    .with_disposition(DeliverySemantics::not_delivered());
    assert_class_b_envelope(
        &error,
        ErrorCode::ActionFailed,
        DeliverySemantics::not_delivered(),
    );
    #[cfg(target_os = "windows")]
    {
        let live = windowless_close_live_error();
        assert_class_b_envelope(
            &live,
            ErrorCode::ActionFailed,
            DeliverySemantics::not_delivered(),
        );
    }
}

#[cfg(target_os = "windows")]
fn windowless_close_live_error() -> AdapterError {
    use std::os::windows::process::CommandExt;
    let mut child = std::process::Command::new("cmd")
        .args(["/C", "ping", "-n", "60", "127.0.0.1", ">", "NUL"])
        .creation_flags(0x0800_0000)
        .spawn()
        .expect("windowless child");
    let pid = ProcessId::from(child.id());
    let started = std::time::Instant::now();
    let token = loop {
        if let Ok(Some(token)) = crate::system::process_identity::token_for_pid(pid) {
            break token;
        }
        if started.elapsed() > std::time::Duration::from_secs(5) {
            panic!("windowless child never exposed a creation-time token");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    let app = AppInfo {
        name: "cmd.exe".into(),
        pid,
        bundle_id: None,
        process_instance: Some(token),
    };
    let error = close_app_impl(&app, false, Deadline::after(5_000).expect("deadline"))
        .expect_err("windowless alive");
    let _ = child.kill();
    let _ = child.wait();
    error
}

#[test]
fn class_b_uipi_activation_refusal_is_perm_denied_not_delivered() {
    let error = activation_elevation_denied();
    assert_class_b_envelope(
        &error,
        ErrorCode::PermDenied,
        DeliverySemantics::not_delivered(),
    );
    assert!(
        error.message.contains("window activation"),
        "activation-worded denial"
    );
    assert_eq!(
        error_wire(&error)["details"]["physical_delivery_started"],
        false
    );
}

#[test]
fn class_b_create_process_invalid_name_is_invalid_args_not_delivered() {
    let error = launch_app_impl(
        r"sub\app.exe",
        &LaunchOptions::default(),
        Deadline::after(1_000).expect("deadline"),
    )
    .expect_err("relative id");
    assert_class_b_envelope(
        &error,
        ErrorCode::InvalidArgs,
        DeliverySemantics::not_delivered(),
    );
}

#[test]
fn fail_closed_higher_integrity_budget_exhaustion_is_perm_denied_not_delivered() {
    let error = budget_exhausted(true);
    assert_code_wire(&error, ErrorCode::PermDenied);
    assert_disposition_wire(&error, DeliverySemantics::not_delivered());
    assert_eq!(
        error_wire(&error)["details"]["physical_delivery_started"],
        false
    );
    assert!(error.message.contains("window activation"));
}

#[test]
fn fail_closed_equal_integrity_budget_exhaustion_is_action_failed_not_started() {
    let error = budget_exhausted(false);
    assert_code_wire(&error, ErrorCode::ActionFailed);
    assert_disposition_wire(&error, DeliverySemantics::not_delivered());
    assert_eq!(
        error_wire(&error)["details"]["physical_delivery_started"],
        false
    );
}

#[test]
fn process_state_serializes_with_tagged_snake_case_shape() {
    assert_eq!(
        serde_json::to_value(ProcessState::Running).expect("ser"),
        serde_json::json!({ "state": "running" })
    );
    assert_eq!(
        serde_json::to_value(ProcessState::Unresponsive).expect("ser"),
        serde_json::json!({ "state": "unresponsive" })
    );
    assert_eq!(
        serde_json::to_value(ProcessState::Exited { code: None }).expect("ser"),
        serde_json::json!({ "state": "exited" })
    );
    assert_eq!(
        serde_json::to_value(ProcessState::Exited { code: Some(0) }).expect("ser"),
        serde_json::json!({ "state": "exited", "code": 0 })
    );
    let crashed = ProcessState::Crashed {
        signal_or_code: 0xC000_0005u32 as i32,
    };
    let value = serde_json::to_value(crashed).expect("ser");
    assert_eq!(value["state"], "crashed");
    assert_eq!(value["signal_or_code"], 0xC000_0005u32 as i32);
}

#[test]
fn stale_focus_token_is_shared_stale_ref_not_delivered() {
    let win = WindowInfo {
        id: "w-1".into(),
        title: String::new(),
        app: "fixture".into(),
        pid: ProcessId::from(1u32),
        process_instance: None,
        bounds: None,
        state: WindowState::default(),
    };
    let lease = InteractionLease::guarded(Deadline::after(1_000).expect("deadline"), ())
        .expect("lease");
    let error = focus_window(&win, &lease).expect_err("stale");
    assert_code_wire(&error, ErrorCode::StaleRef);
    assert_disposition_wire(&error, DeliverySemantics::not_delivered());
}

const LIFECYCLE_COST_ARMS: &[&str] = &["launch_to_window", "close_to_exit", "window_op_round_trip"];

fn assert_lifecycle_cost_capture_spread(label: &str, raw: &str) {
    let value: Value = serde_json::from_str(raw).unwrap_or_else(|err| {
        panic!("{label} must parse as JSON: {err}");
    });
    assert_eq!(value["probe"], "21-system-lifecycle", "{label} probe id");
    let cites = value["methodology_cites"]
        .as_array()
        .unwrap_or_else(|| panic!("{label} missing methodology_cites"));
    assert!(
        cites.iter().any(|entry| entry == "A15-13"),
        "{label} must cite A15-13"
    );
    for arm in LIFECYCLE_COST_ARMS {
        let entry = value
            .get(*arm)
            .unwrap_or_else(|| panic!("{label} missing arm {arm}"));
        let min = entry["min_ms"]
            .as_f64()
            .unwrap_or_else(|| panic!("{label}/{arm} missing min_ms"));
        let median = entry["median_ms"]
            .as_f64()
            .unwrap_or_else(|| panic!("{label}/{arm} missing median_ms"));
        let max = entry["max_ms"]
            .as_f64()
            .unwrap_or_else(|| panic!("{label}/{arm} missing max_ms"));
        assert!(
            min <= median && median <= max,
            "{label}/{arm}: min<=median<=max ({min}, {median}, {max})"
        );
        assert_eq!(entry["n"], 7, "{label}/{arm} n");
        assert_eq!(
            entry["warmup_discarded"], true,
            "{label}/{arm} warmup_discarded"
        );
    }
}

#[test]
fn a20_style_lifecycle_cost_captures_carry_min_median_max_both_environments() {
    assert_lifecycle_cost_capture_spread(
        "lifecycle-cost-devbox",
        include_str!(
            "../../../../probes/windows/21-system-lifecycle/captures/lifecycle-cost-devbox.json"
        ),
    );
    assert_lifecycle_cost_capture_spread(
        "lifecycle-cost-ci",
        include_str!(
            "../../../../probes/windows/21-system-lifecycle/captures/lifecycle-cost-ci.json"
        ),
    );
}
