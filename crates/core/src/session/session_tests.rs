use super::*;
use crate::refs_lock::RefStoreLock;
use crate::refs_test_support::HomeGuard;
use crate::session::{ArtifactsMode, SessionTraceMode};
use std::fs;
use std::time::Duration;

#[test]
fn resolve_prefers_explicit_over_env_and_pointer() {
    let _guard = HomeGuard::new();
    write_current_session_pointer("pointer").unwrap();
    unsafe { std::env::set_var("AGENT_DESKTOP_SESSION", "env-session") };
    let resolved = resolve_active_session(Some("explicit"), None).unwrap();
    assert_eq!(resolved.as_deref(), Some("explicit"));
    unsafe { std::env::remove_var("AGENT_DESKTOP_SESSION") };
}

#[test]
fn resolve_prefers_env_over_pointer() {
    let _guard = HomeGuard::new();
    write_current_session_pointer("pointer").unwrap();
    unsafe { std::env::set_var("AGENT_DESKTOP_SESSION", "env-session") };
    let resolved = resolve_active_session(None, Some("env-session")).unwrap();
    assert_eq!(resolved.as_deref(), Some("env-session"));
    unsafe { std::env::remove_var("AGENT_DESKTOP_SESSION") };
}

#[test]
fn resolve_falls_back_to_pointer() {
    let _guard = HomeGuard::new();
    write_current_session_pointer("pointer").unwrap();
    let resolved = resolve_active_session(None, None).unwrap();
    assert_eq!(resolved.as_deref(), Some("pointer"));
}

#[test]
fn resolve_none_without_pointer() {
    let _guard = HomeGuard::new();
    let resolved = resolve_active_session(None, None).unwrap();
    assert!(resolved.is_none());
}

#[test]
fn manifest_round_trips_with_optional_fields() {
    let _guard = HomeGuard::new();
    let manifest = SessionManifest {
        id: "run-1".into(),
        name: Some("demo".into()),
        created_at: 1,
        ended_at: None,
        trace: SessionTraceMode::On,
        artifacts: ArtifactsMode::Events,
    };
    write_manifest(&manifest).unwrap();
    let loaded = read_manifest("run-1").unwrap().expect("manifest");
    assert_eq!(loaded, manifest);
}

#[test]
fn validate_session_name_rejects_control_chars() {
    let err = validate_session_name("bad\u{1}name").unwrap_err();
    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn start_creates_tree_manifest_and_pointer() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        name: Some("demo".into()),
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();
    assert!(session_dir(&manifest.id).unwrap().join("trace").is_dir());
    assert_eq!(
        read_current_session_pointer().unwrap().as_deref(),
        Some(manifest.id.as_str())
    );
}

#[test]
fn start_refuses_live_pointer_without_force() {
    let _guard = HomeGuard::new();
    let first = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();
    let _lock = RefStoreLock::acquire(
        &crate::refs_store::RefStore::for_session(Some(&first.id))
            .unwrap()
            .base_dir()
            .join("refstore.lock"),
    )
    .unwrap();
    let err = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn end_seals_manifest_and_clears_pointer() {
    let _guard = HomeGuard::new();
    let _manifest = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();
    let ended = end_session(None).unwrap();
    assert!(ended.ended_at.is_some());
    assert!(read_current_session_pointer().unwrap().is_none());
}

#[test]
fn gc_removes_ended_sessions_but_not_pointer_or_live() {
    let _guard = HomeGuard::new();
    let live = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();
    let ended = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: true,
        ..Default::default()
    })
    .unwrap();
    end_session(Some(&ended.id)).unwrap();
    let report = gc(GcOptions {
        ended_only: false,
        older_than: None,
    })
    .unwrap();
    assert!(report.removed.contains(&ended.id));
    assert!(!report.removed.contains(&live.id));
    assert!(session_dir(&live.id).unwrap().is_dir());
    assert!(!session_dir(&ended.id).unwrap().exists());
}

#[test]
#[cfg(unix)]
fn remove_session_dir_rejects_symlink() {
    let _guard = HomeGuard::new();
    let dir = session_dir("symlink-session").unwrap();
    let target = dir.with_extension("target");
    fs::create_dir_all(&target).unwrap();
    std::os::unix::fs::symlink(&target, &dir).unwrap();
    let err = super::gc::remove_session_dir(&dir).unwrap_err();
    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn list_reports_manifest_fields_only() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        name: Some("listed".into()),
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();
    let listed = list_sessions().unwrap();
    assert!(listed.iter().any(|entry| entry.id == manifest.id));
}

#[test]
fn trace_enabled_requires_manifest_on() {
    let _guard = HomeGuard::new();
    assert!(!trace_enabled_for_session("missing").unwrap());
    let manifest = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::Off,
        force: false,
        ..Default::default()
    })
    .unwrap();
    assert!(!trace_enabled_for_session(&manifest.id).unwrap());
}

#[test]
fn gc_respects_older_than_threshold() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::Off,
        force: false,
        ..Default::default()
    })
    .unwrap();
    end_session(Some(&manifest.id)).unwrap();
    clear_current_session_pointer().unwrap();
    let report = gc(GcOptions {
        ended_only: false,
        older_than: Some(Duration::from_secs(3600)),
    })
    .unwrap();
    assert!(report.removed.is_empty());
}

#[test]
fn new_session_id_includes_process_id() {
    let id = new_session_id();
    assert!(id.contains(&std::process::id().to_string()));
    validate_session_id(&id).expect("new_session_id must always be a valid session id");
}

#[test]
fn corrupt_manifest_is_ignored_not_fatal() {
    let _guard = HomeGuard::new();
    let good = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::Off,
        force: true,
        ..Default::default()
    })
    .unwrap();
    let bad_dir = session_dir("corruptsess").unwrap();
    fs::create_dir_all(&bad_dir).unwrap();
    fs::write(bad_dir.join("session.json"), b"{ not valid json").unwrap();

    assert!(!trace_enabled_for_session("corruptsess").unwrap());
    let listed: Vec<String> = list_sessions().unwrap().into_iter().map(|m| m.id).collect();
    assert!(listed.contains(&good.id));
    assert!(!listed.iter().any(|id| id == "corruptsess"));
}

#[cfg(unix)]
#[test]
fn unreadable_manifest_is_skipped_not_fatal_for_list_and_gc() {
    use std::os::unix::fs::PermissionsExt;

    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let _guard = HomeGuard::new();
    let good = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::Off,
        force: true,
        ..Default::default()
    })
    .unwrap();
    let bad_dir = session_dir("unreadablesess").unwrap();
    fs::create_dir_all(&bad_dir).unwrap();
    let manifest = bad_dir.join("session.json");
    fs::write(&manifest, b"{}").unwrap();
    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o000)).unwrap();

    let listed: Vec<String> = list_sessions().unwrap().into_iter().map(|m| m.id).collect();
    assert!(listed.contains(&good.id));
    assert!(!listed.iter().any(|id| id == "unreadablesess"));
    assert!(read_manifest("unreadablesess").unwrap().is_none());

    fs::set_permissions(&manifest, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn gc_leaves_recently_created_unended_session() {
    let _guard = HomeGuard::new();
    let started = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::Off,
        force: true,
        ..Default::default()
    })
    .unwrap();
    clear_current_session_pointer().unwrap();
    let report = gc(GcOptions {
        ended_only: false,
        older_than: Some(Duration::from_secs(0)),
    })
    .unwrap();
    assert!(!report.removed.contains(&started.id));
    assert!(session_dir(&started.id).unwrap().is_dir());
}

#[test]
fn start_with_force_overrides_live_pointer() {
    let _guard = HomeGuard::new();
    let first = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();
    let _lock = RefStoreLock::acquire(
        &crate::refs_store::RefStore::for_session(Some(&first.id))
            .unwrap()
            .base_dir()
            .join("refstore.lock"),
    )
    .unwrap();
    let second = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: true,
        ..Default::default()
    })
    .unwrap();
    assert_ne!(first.id, second.id);
    assert_eq!(
        read_current_session_pointer().unwrap().as_deref(),
        Some(second.id.as_str())
    );
}

#[test]
fn trace_enabled_false_once_session_ended() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();
    assert!(trace_enabled_for_session(&manifest.id).unwrap());
    end_session(Some(&manifest.id)).unwrap();
    assert!(!trace_enabled_for_session(&manifest.id).unwrap());
}

#[test]
fn start_with_screenshots_records_full_artifacts_mode() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        artifacts: ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap();
    assert_eq!(manifest.artifacts, ArtifactsMode::Full);
    let loaded = read_manifest(&manifest.id).unwrap().expect("manifest");
    assert_eq!(loaded.artifacts, ArtifactsMode::Full);
}

#[test]
fn start_without_screenshots_records_events_artifacts_mode() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        ..Default::default()
    })
    .unwrap();
    assert_eq!(manifest.artifacts, ArtifactsMode::Events);
}

#[test]
fn legacy_manifest_without_artifacts_defaults_to_events() {
    let _guard = HomeGuard::new();
    let dir = session_dir("legacy").unwrap();
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("session.json"),
        r#"{"id":"legacy","created_at":1,"trace":"on"}"#,
    )
    .unwrap();
    let manifest = read_manifest("legacy").unwrap().expect("manifest");
    assert_eq!(manifest.artifacts, ArtifactsMode::Events);
}

#[test]
fn no_trace_with_screenshots_is_invalid_args() {
    let _guard = HomeGuard::new();
    let err = start_session(StartSessionOptions {
        trace: SessionTraceMode::Off,
        artifacts: ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap_err();
    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn ended_session_reports_artifacts_full_false() {
    let _guard = HomeGuard::new();
    let manifest = start_session(StartSessionOptions {
        artifacts: ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap();
    assert!(manifest.artifacts_full());
    end_session(Some(&manifest.id)).unwrap();
    let ended = read_manifest(&manifest.id).unwrap().expect("manifest");
    assert!(!ended.artifacts_full());
}

#[cfg(unix)]
#[test]
fn symlinked_manifest_is_ignored_not_fatal() {
    let _guard = HomeGuard::new();
    let good = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::Off,
        force: true,
        ..Default::default()
    })
    .unwrap();
    let dir = session_dir("symsess").unwrap();
    fs::create_dir_all(&dir).unwrap();
    let target = dir.with_extension("target");
    fs::write(&target, b"{}").unwrap();
    std::os::unix::fs::symlink(&target, dir.join("session.json")).unwrap();

    assert!(!trace_enabled_for_session("symsess").unwrap());
    let ids: Vec<String> = list_sessions().unwrap().into_iter().map(|m| m.id).collect();
    assert!(ids.contains(&good.id));
    assert!(!ids.iter().any(|id| id == "symsess"));
}

#[cfg(unix)]
#[test]
fn symlinked_session_pointer_degrades_to_none() {
    let _guard = HomeGuard::new();
    let target = agent_desktop_dir().unwrap().join("pointer-target");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(&target, b"whatever").unwrap();
    std::os::unix::fs::symlink(&target, current_session_path().unwrap()).unwrap();

    assert!(read_current_session_pointer().unwrap().is_none());
}
