use super::*;
use crate::action::Action;
use crate::action_request::ActionRequest;
use crate::action_result::ActionResult;
use crate::adapter::{ImageBuffer, ImageFormat, NativeHandle, PlatformAdapter, ScreenshotTarget};
use crate::context::CommandContext;
use crate::error::AdapterError;
use crate::ref_action::{ResolvedRefAction, execute_resolved};
use crate::refs::RefMap;
use crate::refs_store::RefStore;
use crate::refs_test_support::HomeGuard;
use crate::session::{ArtifactsMode, SessionTraceMode, StartSessionOptions, start_session};
use crate::trace_artifacts::clear_test_budgets;
use crate::{capability, refs::RefEntry};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

const MINI_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

fn entry(pid: i32) -> RefEntry {
    RefEntry {
        pid,
        role: "button".into(),
        name: Some("Run".into()),
        value: None,
        description: None,
        states: vec![],
        bounds: None,
        bounds_hash: None,
        available_actions: vec![capability::CLICK.into()],
        source_app: None,
        source_window_id: None,
        source_window_title: None,
        source_surface: crate::adapter::SnapshotSurface::Window,
        root_ref: None,
        path_is_absolute: false,
        path: smallvec::SmallVec::new(),
    }
}

fn artifacts_session() -> crate::session::SessionManifest {
    start_session(StartSessionOptions {
        trace: SessionTraceMode::On,
        artifacts: ArtifactsMode::Full,
        ..Default::default()
    })
    .unwrap()
}

struct PngAdapter {
    target: Mutex<Option<ScreenshotTarget>>,
}

impl PlatformAdapter for PngAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("ok"))
    }

    fn screenshot(&self, target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> {
        *self.target.lock().unwrap() = Some(target);
        Ok(ImageBuffer {
            data: MINI_PNG.to_vec(),
            format: ImageFormat::Png,
            width: 1,
            height: 1,
        })
    }
}

struct FailingScreenshotAdapter;

impl PlatformAdapter for FailingScreenshotAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("ok"))
    }

    fn screenshot(&self, _target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> {
        Err(AdapterError::not_supported("screenshot"))
    }
}

struct FailingActionAdapter {
    screenshot_calls: AtomicU32,
}

impl PlatformAdapter for FailingActionAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::internal("boom"))
    }

    fn screenshot(&self, _target: ScreenshotTarget) -> Result<ImageBuffer, AdapterError> {
        self.screenshot_calls.fetch_add(1, Ordering::SeqCst);
        Ok(ImageBuffer {
            data: MINI_PNG.to_vec(),
            format: ImageFormat::Png,
            width: 1,
            height: 1,
        })
    }
}

struct DefaultScreenshotAdapter;

impl PlatformAdapter for DefaultScreenshotAdapter {
    fn resolve_element_strict(&self, _entry: &RefEntry) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::new("ok"))
    }
}

fn run_ref_action(
    context: &CommandContext,
    adapter: &dyn PlatformAdapter,
    pid: i32,
) -> Result<ActionResult, crate::error::AppError> {
    let entry = entry(pid);
    execute_resolved(
        ResolvedRefAction {
            adapter,
            entry: &entry,
            handle: &NativeHandle::null(),
            ref_id: "@e1",
            context,
        },
        ActionRequest::headless(Action::Click),
    )
}

static ARTIFACT_TEST_LOCK: Mutex<()> = Mutex::new(());

fn setup_artifacts_test() -> (HomeGuard, std::sync::MutexGuard<'static, ()>) {
    clear_test_budgets();
    (
        HomeGuard::new(),
        ARTIFACT_TEST_LOCK.lock().unwrap_or_else(|p| p.into_inner()),
    )
}

#[test]
fn artifacts_full_captures_pre_and_post_pngs() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let adapter = PngAdapter {
        target: Mutex::new(None),
    };
    run_ref_action(&context, &adapter, 42).unwrap();
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
fn events_mode_produces_no_artifact_files() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = start_session(StartSessionOptions {
        trace: SessionTraceMode::On,
        ..Default::default()
    })
    .unwrap();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let adapter = PngAdapter {
        target: Mutex::new(None),
    };
    run_ref_action(&context, &adapter, 1).unwrap();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    assert!(!trace_dir.join("screens").exists());
}

#[test]
fn trace_off_with_artifacts_full_captures_nothing() {
    let (_home, _lock) = setup_artifacts_test();
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-artifacts-off-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let context = CommandContext::new(None, Some(path.clone()), true).unwrap();
    let adapter = PngAdapter {
        target: Mutex::new(None),
    };
    run_ref_action(&context, &adapter, 1).unwrap();
    assert!(!path.parent().unwrap().join("screens").exists());
    let _ = std::fs::remove_file(path);
}

#[test]
fn adapter_screenshot_error_still_succeeds_with_skip_reason() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    run_ref_action(&context, &FailingScreenshotAdapter, 1).unwrap();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    let segment = std::fs::read_dir(&trace_dir)
        .unwrap()
        .find_map(|e| {
            let p = e.ok()?.path();
            p.extension().is_some_and(|ext| ext == "jsonl").then_some(p)
        })
        .unwrap();
    let body = std::fs::read_to_string(segment).unwrap();
    assert!(body.contains("adapter:"));
    clear_test_budgets();
}

#[test]
fn byte_budget_exhaustion_skips_with_budget_reason() {
    let (_home, _lock) = setup_artifacts_test();
    set_test_budgets(10, 200, 64 * 1024 * 1024);
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    run_ref_action(
        &context,
        &PngAdapter {
            target: Mutex::new(None),
        },
        1,
    )
    .unwrap();
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
fn count_budget_exhaustion_skips_with_count_budget_reason() {
    let (_home, _lock) = setup_artifacts_test();
    set_test_budgets(128 * 1024 * 1024, 1, 64 * 1024 * 1024);
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let adapter = PngAdapter {
        target: Mutex::new(None),
    };
    run_ref_action(&context, &adapter, 1).unwrap();
    run_ref_action(&context, &adapter, 1).unwrap();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    let count = std::fs::read_dir(trace_dir.join("screens"))
        .map(|d| d.count())
        .unwrap_or(0);
    assert_eq!(count, 1);
    clear_test_budgets();
}

#[cfg(unix)]
#[test]
fn symlinked_screens_dir_refuses_capture() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let trace_dir = RefStore::for_session(Some(&manifest.id))
        .unwrap()
        .trace_dir();
    std::fs::create_dir_all(&trace_dir).unwrap();
    let outside = trace_dir.join("outside-screens");
    std::fs::create_dir_all(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, trace_dir.join("screens")).unwrap();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    run_ref_action(
        &context,
        &PngAdapter {
            target: Mutex::new(None),
        },
        1,
    )
    .unwrap();
    assert_eq!(
        std::fs::read_dir(outside).map(|d| d.count()).unwrap_or(0),
        0
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
    copy_refmap_if_full(&context, &store, &snapshot_id).unwrap();
    copy_refmap_if_full(&context, &store, &snapshot_id).unwrap();
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
    copy_refmap_if_full(&context, &store, &first).unwrap();
    let copied = store
        .trace_dir()
        .join("refmaps")
        .join(format!("{first}.json"));
    assert!(copied.is_file());
    for i in 0..600 {
        let mut map = RefMap::new();
        map.allocate(entry(i));
        let id = store.save_new_snapshot(&map).unwrap();
        copy_refmap_if_full(&context, &store, &id).unwrap();
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
fn default_adapter_screenshot_skips_cleanly() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    run_ref_action(&context, &DefaultScreenshotAdapter, 1).unwrap();
}

#[test]
fn concurrent_capture_produces_distinct_filenames() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let session_home = crate::refs::home_dir().unwrap();
    let context = Arc::new(CommandContext::new(Some(manifest.id.clone()), None, false).unwrap());
    let adapter = Arc::new(PngAdapter {
        target: Mutex::new(None),
    });
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
fn failing_action_still_captures_post_screenshot() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let adapter = FailingActionAdapter {
        screenshot_calls: AtomicU32::new(0),
    };
    let err = run_ref_action(&context, &adapter, 1).unwrap_err();
    assert_eq!(err.code(), "INTERNAL");
    assert_eq!(adapter.screenshot_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn capture_targets_window_for_pid() {
    let (_home, _lock) = setup_artifacts_test();
    let manifest = artifacts_session();
    let context = CommandContext::new(Some(manifest.id.clone()), None, false).unwrap();
    let adapter = PngAdapter {
        target: Mutex::new(None),
    };
    run_ref_action(&context, &adapter, 99).unwrap();
    match adapter.target.lock().unwrap().take() {
        Some(ScreenshotTarget::Window(pid)) => assert_eq!(pid, 99),
        _ => panic!("expected Window screenshot target"),
    }
}
