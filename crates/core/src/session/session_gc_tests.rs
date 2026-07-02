use super::*;
use crate::refs_test_support::HomeGuard;
use crate::session::SessionTraceMode;
use std::fs;
use std::time::Duration;

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
