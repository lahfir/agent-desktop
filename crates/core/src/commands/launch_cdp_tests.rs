use super::cdp_test_support::{CdpAdapter, adapter_error, empty_cdp_adapter};
use super::*;
use crate::launch_options::LaunchOptions;
use crate::{AppInfo, ErrorCode, ProcessId, RendererKind};
use std::sync::atomic::Ordering;

#[test]
fn cdp_launch_rejects_a_conflicting_remote_debugging_switch() {
    let adapter = empty_cdp_adapter(false);
    let options = LaunchOptions {
        args: vec!["--remote-debugging-port=1234".into()],
        cdp_port: Some(0),
        ..Default::default()
    };

    let error = adapter_error(
        execute(
            LaunchArgs {
                app: "Fixture".into(),
                options,
            },
            &adapter,
        )
        .unwrap_err(),
    );

    assert_eq!(error.code, ErrorCode::InvalidArgs);
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("kind")),
        Some(&serde_json::json!("cdp_switch_conflict"))
    );
    assert_eq!(
        adapter.list_apps_calls.load(Ordering::SeqCst),
        0,
        "the switch conflict is rejected before the running-app precheck runs"
    );
}

#[test]
fn cdp_launch_rejects_an_already_running_instance() {
    let adapter = CdpAdapter {
        running: vec![AppInfo {
            name: "Fixture".into(),
            pid: ProcessId::new(7),
            bundle_id: None,
            process_instance: Some("7:1".into()),
            presentation: None,
        }],
        ..empty_cdp_adapter(false)
    };
    let options = LaunchOptions {
        cdp_port: Some(0),
        ..Default::default()
    };

    let error = adapter_error(
        execute(
            LaunchArgs {
                app: "Fixture".into(),
                options,
            },
            &adapter,
        )
        .unwrap_err(),
    );

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("kind")),
        Some(&serde_json::json!("cdp_requires_fresh_launch"))
    );
    assert_eq!(adapter.list_apps_calls.load(Ordering::SeqCst), 1);
}

/// Even when the endpoint never answers, the switch was already appended
/// and the resolved port already recorded before `launch_app` ran — the
/// failure is in verification, not in requesting the port.
#[test]
fn cdp_launch_appends_the_switch_and_resolves_the_port_before_verification_fails() {
    let adapter = empty_cdp_adapter(false);
    let options = LaunchOptions {
        cdp_port: Some(0),
        timeout_ms: 200,
        ..Default::default()
    };

    let error = adapter_error(
        execute(
            LaunchArgs {
                app: "Fixture".into(),
                options,
            },
            &adapter,
        )
        .unwrap_err(),
    );

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error
            .details
            .as_ref()
            .and_then(|details| details.get("kind")),
        Some(&serde_json::json!("cdp_endpoint_unavailable"))
    );
    let captured_port = adapter
        .captured_cdp_port
        .lock()
        .unwrap()
        .expect("cdp_port was resolved before launch_app ran");
    let captured_args = adapter.captured_args.lock().unwrap();
    assert!(captured_args.contains(&format!("--remote-debugging-port={captured_port}")));
}

#[test]
fn cdp_launch_returns_a_verified_endpoint_when_the_process_answers() {
    let adapter = empty_cdp_adapter(true);
    let options = LaunchOptions {
        cdp_port: Some(0),
        timeout_ms: 3_000,
        ..Default::default()
    };

    let value = execute(
        LaunchArgs {
            app: "Fixture".into(),
            options,
        },
        &adapter,
    )
    .unwrap();

    let cdp = value
        .get("cdp")
        .expect("cdp endpoint present in the envelope");
    assert_eq!(
        cdp.get("product").and_then(Value::as_str),
        Some("Fixture/1.0")
    );
    assert!(
        cdp.get("websocket_url")
            .and_then(Value::as_str)
            .unwrap()
            .contains("devtools/browser")
    );
}

/// A Chromium app launched without `--cdp` gets nudged toward the flag that
/// actually opens the web-content door, without repeating that door's name.
#[test]
fn chromium_renderer_without_cdp_gets_the_relaunch_suggestion() {
    let adapter = CdpAdapter {
        renderer: Some(RendererKind::Chromium),
        ..empty_cdp_adapter(false)
    };

    let value = execute(
        LaunchArgs {
            app: "Fixture".into(),
            options: LaunchOptions::default(),
        },
        &adapter,
    )
    .unwrap();

    assert_eq!(
        value.get("renderer").and_then(Value::as_str),
        Some("chromium")
    );
    assert!(value.get("cdp").is_none());
    let suggestion = value
        .get("suggestion")
        .and_then(Value::as_str)
        .expect("suggestion present for an undriven chromium renderer");
    assert!(suggestion.contains("--cdp"));
}

#[test]
fn cdp_success_gets_the_handoff_suggestion() {
    let adapter = empty_cdp_adapter(true);
    let options = LaunchOptions {
        cdp_port: Some(0),
        timeout_ms: 3_000,
        ..Default::default()
    };

    let value = execute(
        LaunchArgs {
            app: "Fixture".into(),
            options,
        },
        &adapter,
    )
    .unwrap();

    assert!(value.get("cdp").is_some());
    let suggestion = value
        .get("suggestion")
        .and_then(Value::as_str)
        .expect("suggestion present once the endpoint is verified");
    assert!(suggestion.contains("agent-browser connect"));
    assert!(suggestion.contains("Do not hand-roll raw CDP"));
}

#[test]
fn non_chromium_plain_launch_has_neither_renderer_nor_suggestion() {
    let adapter = empty_cdp_adapter(false);

    let value = execute(
        LaunchArgs {
            app: "Fixture".into(),
            options: LaunchOptions::default(),
        },
        &adapter,
    )
    .unwrap();

    assert!(value.get("renderer").is_none());
    assert!(value.get("suggestion").is_none());
}
