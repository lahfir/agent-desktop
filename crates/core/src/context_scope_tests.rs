use super::*;
use crate::error::AppError;
use crate::session::{SessionTraceMode, StartSessionOptions, start_session};
use serde_json::json;

#[test]
fn command_scope_emits_start_and_success_end() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-scope-ok-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let context = CommandContext::new(None, Some(path.clone()), true).unwrap();
    let scope = context.command_scope("snapshot");
    scope.complete(&Ok(json!({ "ok": true })));

    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains(r#""event":"command.start""#));
    assert!(body.contains(r#""event":"command.end""#));
    assert!(body.contains(r#""ok":true"#));
    let end_line = body
        .lines()
        .find(|line| line.contains(r#""event":"command.end""#))
        .expect("command.end line");
    let end_event: serde_json::Value = serde_json::from_str(end_line).unwrap();
    assert!(end_event["duration_ms"].as_u64().is_some());
    let _ = std::fs::remove_file(path);
}

#[test]
fn command_scope_emits_error_end_with_code_and_message() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-scope-err-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let context = CommandContext::new(None, Some(path.clone()), true).unwrap();
    let scope = context.command_scope("wait");
    let err = AppError::invalid_input("bad args");
    scope.complete(&Err(err));

    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains(r#""ok":false"#));
    assert!(body.contains(r#""code":"INVALID_ARGS""#));
    let _ = std::fs::remove_file(path);
}

#[test]
fn command_scope_drop_emits_internal_end_once() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-scope-drop-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let context = CommandContext::new(None, Some(path.clone()), true).unwrap();
    {
        let _scope = context.command_scope("click");
    }

    let body = std::fs::read_to_string(&path).unwrap();
    assert_eq!(body.matches(r#""event":"command.end""#).count(), 1);
    assert!(body.contains(r#""code":"INTERNAL""#));
    let _ = std::fs::remove_file(path);
}

#[test]
fn command_scope_is_noop_without_trace_sink() {
    let context = CommandContext::default();
    let scope = context.command_scope("status");
    scope.complete(&Ok(json!({})));
}

#[test]
fn artifacts_full_follows_manifest_mode() {
    let _guard = crate::refs_test_support::HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        trace: SessionTraceMode::On,
        artifacts: crate::session::ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    assert!(context.artifacts_full());
}

#[test]
fn wait_text_timeout_message_omits_raw_text_from_trace_segment() {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-scope-wait-text-redact-{}.jsonl",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let context = CommandContext::new(None, Some(path.clone()), true).unwrap();
    let marker = "zzq93f_super_secret_marker_do_not_leak";
    let err = crate::commands::wait_timeout::text(marker, 50, None, None).unwrap_err();
    let scope = context.command_scope("wait");
    scope.complete(&Err(err));

    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains(r#""event":"command.end""#));
    assert!(!body.contains(marker));
    let _ = std::fs::remove_file(path);
}
