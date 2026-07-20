use super::*;
use crate::refs_test_support::HomeGuard;
use crate::session::SessionTraceMode;
#[cfg(unix)]
use std::fs;
use std::time::Duration;

#[test]
fn multiple_liveness_owners_do_not_serialize_each_other() {
    let _guard = HomeGuard::new();
    let mut manifest = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::Off,
        ..Default::default()
    })
    .unwrap();
    manifest.created_at = 0;
    write_manifest(&manifest).unwrap();

    let first = acquire_liveness_lease(&manifest.id).unwrap().unwrap();
    let second = acquire_liveness_lease(&manifest.id).unwrap().unwrap();
    drop(first);

    assert!(is_live(&manifest.id).unwrap());
    assert!(session_dir(&manifest.id).unwrap().is_dir());

    drop(second);
    let report = gc(GcOptions {
        ended_only: false,
        older_than: Some(Duration::ZERO),
    })
    .unwrap();
    assert!(report.removed.contains(&manifest.id));
}

#[test]
fn subprocess_session_lease_holder() {
    let Some(ready) = std::env::var_os("AGENT_DESKTOP_SESSION_LEASE_READY") else {
        return;
    };
    let session_id = std::env::var("AGENT_DESKTOP_SESSION_LEASE_ID").unwrap();
    let _lease = acquire_liveness_lease(&session_id).unwrap().unwrap();
    std::fs::write(ready, b"ready").unwrap();
    std::thread::sleep(Duration::from_secs(5));
}

#[test]
fn gc_retains_idle_cross_process_owner_and_reaps_after_crash() {
    let guard = HomeGuard::new();
    let mut manifest = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::Off,
        ..Default::default()
    })
    .unwrap();
    manifest.created_at = 0;
    write_manifest(&manifest).unwrap();
    let ready = guard.path().join("lease-ready");
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("session::gc_tests::subprocess_session_lease_holder")
        .arg("--nocapture")
        .env("HOME", guard.path())
        .env("AGENT_DESKTOP_SESSION_LEASE_READY", &ready)
        .env("AGENT_DESKTOP_SESSION_LEASE_ID", &manifest.id)
        .spawn()
        .unwrap();
    let ready_deadline = std::time::Instant::now() + Duration::from_secs(2);
    while !ready.is_file() && std::time::Instant::now() < ready_deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        ready.is_file(),
        "subprocess did not acquire its session lease"
    );

    let retained = gc(GcOptions {
        ended_only: false,
        older_than: Some(Duration::ZERO),
    })
    .unwrap();
    assert!(!retained.removed.contains(&manifest.id));

    child.kill().unwrap();
    child.wait().unwrap();
    let removed = gc(GcOptions {
        ended_only: false,
        older_than: Some(Duration::ZERO),
    })
    .unwrap();
    assert!(removed.removed.contains(&manifest.id));
}

#[test]
fn gc_removes_ended_sessions_but_not_pointer_or_live() {
    let _guard = HomeGuard::new();
    let live = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        ..Default::default()
    })
    .unwrap();
    let ended = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        ..Default::default()
    })
    .unwrap();
    end_session(&ended.id).unwrap();
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
        ..Default::default()
    })
    .unwrap();
    end_session(&manifest.id).unwrap();
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
        ..Default::default()
    })
    .unwrap();
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
