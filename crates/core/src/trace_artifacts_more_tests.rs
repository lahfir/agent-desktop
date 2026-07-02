use super::tests::{artifacts_session, entry, png_adapter, run_ref_action, setup_artifacts_test};
use super::*;
use crate::context::CommandContext;
use crate::refs::RefMap;
use crate::refs_store::RefStore;
use crate::trace_artifacts::clear_test_budgets;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

#[test]
fn artifacts_full_captures_pre_and_post_pngs() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    run_ref_action(&context, &png_adapter(), 42).unwrap();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    let screens: Vec<_> = std::fs::read_dir(trace_dir.join("screens"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(screens.len(), 2);
    for path in &screens {
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    let segments: Vec<_> = std::fs::read_dir(&trace_dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    let body = std::fs::read_to_string(segments[0].clone()).unwrap();
    assert!(body.contains("action.artifacts"));
    assert!(body.contains("screens/"));
}

#[test]
fn byte_budget_exhaustion_skips_with_budget_reason() {
    let (_home, _lock) = setup_artifacts_test();
    set_test_budgets(10, 200, 64 * 1024 * 1024);
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    run_ref_action(&context, &png_adapter(), 1).unwrap();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    assert_eq!(
        std::fs::read_dir(trace_dir.join("screens"))
            .map(|dir| dir.count())
            .unwrap_or(0),
        0
    );
    let body = std::fs::read_dir(&trace_dir)
        .unwrap()
        .find_map(|e| {
            let p = e.ok()?.path();
            p.extension()
                .is_some_and(|ext| ext == "jsonl")
                .then_some(std::fs::read_to_string(p).ok())
        })
        .flatten()
        .unwrap();
    assert!(body.contains("\"skipped\":\"budget\"") || body.contains("skipped_pre"));
    clear_test_budgets();
}

#[test]
fn concurrent_capture_produces_distinct_filenames() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let session_home = crate::refs::home_dir().unwrap();
    let context = Arc::new(CommandContext::new(Some(manifest.id.clone()), None, false).unwrap());
    let adapter = Arc::new(png_adapter());
    let handles: Vec<_> = (0..4)
        .map(|_| {
            let context = context.clone();
            let adapter = adapter.clone();
            let session_home = session_home.clone();
            std::thread::spawn(move || {
                crate::refs::set_home_override(Some(session_home));
                run_ref_action(&context, adapter.as_ref(), 7)
            })
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    let names: Vec<_> = std::fs::read_dir(
        RefStore::for_session(Some(&manifest.id))
            .unwrap()
            .trace_dir()
            .join("screens"),
    )
    .unwrap()
    .map(|e| e.unwrap().file_name())
    .collect();
    assert_eq!(
        names.len(),
        names.iter().collect::<std::collections::HashSet<_>>().len()
    );
}

#[test]
fn refmap_copy_is_idempotent_and_byte_equal() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let store = RefStore::for_session(Some(&manifest.id)).unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(entry(1));
    let snapshot_id = store.save_new_snapshot(&refmap).unwrap();
    copy_refmap_if_full(&context, &store, &snapshot_id, &refmap).unwrap();
    copy_refmap_if_full(&context, &store, &snapshot_id, &refmap).unwrap();
    let copied = store
        .trace_dir()
        .join("refmaps")
        .join(format!("{snapshot_id}.json"));
    let source = store
        .base_dir()
        .join("snapshots")
        .join(&snapshot_id)
        .join("refmap.json");
    assert_eq!(
        std::fs::read(copied).unwrap(),
        std::fs::read(source).unwrap()
    );
}

#[test]
fn refmap_budget_skip_then_prune_leaves_prior_copy() {
    let (_home, _lock) = setup_artifacts_test();
    set_test_budgets(128 * 1024 * 1024, 200, 8192);
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let store = RefStore::for_session(Some(&manifest.id)).unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(entry(1));
    let first = store.save_new_snapshot(&refmap).unwrap();
    copy_refmap_if_full(&context, &store, &first, &refmap).unwrap();
    let copied = store
        .trace_dir()
        .join("refmaps")
        .join(format!("{first}.json"));
    assert!(copied.is_file());
    for i in 0..600 {
        let mut map = RefMap::new();
        map.allocate(entry(i));
        let id = store.save_new_snapshot(&map).unwrap();
        copy_refmap_if_full(&context, &store, &id, &map).unwrap();
    }
    assert!(copied.is_file());
    assert!(
        !store
            .base_dir()
            .join("snapshots")
            .join(&first)
            .join("refmap.json")
            .is_file()
    );
    clear_test_budgets();
}

#[test]
fn reserve_atomic_bytes_never_overshoots_under_concurrency() {
    let used = Arc::new(AtomicU64::new(0));
    let byte_len: u64 = 7;
    let limit: u64 = byte_len * 10;
    let mut handles = Vec::with_capacity(100);
    for _ in 0..100 {
        let used = used.clone();
        handles.push(std::thread::spawn(move || {
            reserve_atomic_bytes(&used, limit, byte_len).is_ok()
        }));
    }
    let successes = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|ok| *ok)
        .count() as u64;
    assert!(used.load(Ordering::Relaxed) <= limit);
    assert!(successes * byte_len <= limit);
    assert_eq!(successes, 10);
}

#[test]
fn reserve_atomic_count_never_overshoots_under_concurrency() {
    let used = Arc::new(AtomicU32::new(0));
    let limit: u32 = 10;
    let mut handles = Vec::with_capacity(100);
    for _ in 0..100 {
        let used = used.clone();
        handles.push(std::thread::spawn(move || {
            reserve_atomic_count(&used, limit).is_ok()
        }));
    }
    let successes = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .filter(|ok| *ok)
        .count() as u32;
    assert!(used.load(Ordering::Relaxed) <= limit);
    assert_eq!(successes, limit);
}

#[test]
fn copy_refmap_internal_failure_is_best_effort() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let store = RefStore::for_session(Some(&manifest.id)).unwrap();
    let mut refmap = RefMap::new();
    refmap.allocate(entry(1));
    let snapshot_id = store.save_new_snapshot(&refmap).unwrap();

    let trace_dir = store.trace_dir();
    std::fs::create_dir_all(&trace_dir).unwrap();
    let refmaps_path = trace_dir.join("refmaps");
    std::fs::write(&refmaps_path, b"blocking file, not a directory").unwrap();

    let result = copy_refmap_if_full(&context, &store, &snapshot_id, &refmap);

    assert!(
        result.is_ok(),
        "artifact capture must never fail the primary command: {result:?}"
    );
    assert!(
        refmaps_path.is_file(),
        "blocking file should be left untouched by the best-effort skip"
    );
}
