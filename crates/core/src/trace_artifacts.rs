use crate::{
    adapter::{PlatformAdapter, ScreenshotTarget},
    context::CommandContext,
    error::AppError,
    refs::write_private_file,
    refs_store::RefStore,
    trace::ensure_trace_dir,
};
use serde_json::json;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};

const SCREENSHOT_BYTE_BUDGET: u64 = 128 * 1024 * 1024;
const SCREENSHOT_COUNT_BUDGET: u32 = 200;
const REFMAP_BYTE_BUDGET: u64 = 64 * 1024 * 1024;

static CAPTURE_SEQ: AtomicU32 = AtomicU32::new(0);
static SCREENSHOT_BYTE_LIMIT: AtomicU64 = AtomicU64::new(SCREENSHOT_BYTE_BUDGET);
static SCREENSHOT_COUNT_LIMIT: AtomicU32 = AtomicU32::new(SCREENSHOT_COUNT_BUDGET);
static REFMAP_BYTE_LIMIT: AtomicU64 = AtomicU64::new(REFMAP_BYTE_BUDGET);
static SCREENSHOT_BYTES_USED: AtomicU64 = AtomicU64::new(0);
static SCREENSHOT_COUNT_USED: AtomicU32 = AtomicU32::new(0);
static REFMAP_BYTES_USED: AtomicU64 = AtomicU64::new(0);

#[cfg(test)]
#[derive(Clone, Copy)]
struct LocalBudget {
    screenshot_bytes: u64,
    screenshot_count: u32,
    refmap_bytes: u64,
    screenshot_bytes_used: u64,
    screenshot_count_used: u32,
    refmap_bytes_used: u64,
}

#[cfg(test)]
thread_local! {
    static LOCAL_BUDGET: std::cell::RefCell<Option<LocalBudget>> = const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_test_budgets(screenshot_bytes: u64, screenshot_count: u32, refmap_bytes: u64) {
    LOCAL_BUDGET.with(|cell| {
        *cell.borrow_mut() = Some(LocalBudget {
            screenshot_bytes,
            screenshot_count,
            refmap_bytes,
            screenshot_bytes_used: 0,
            screenshot_count_used: 0,
            refmap_bytes_used: 0,
        });
    });
}

#[cfg(test)]
pub(crate) fn clear_test_budgets() {
    LOCAL_BUDGET.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

#[cfg(test)]
fn reserve_screenshot_local(byte_len: u64) -> Option<Result<(), &'static str>> {
    let mut local = LOCAL_BUDGET.with(|cell| *cell.borrow())?;
    if local.screenshot_count_used >= local.screenshot_count {
        return Some(Err("count_budget"));
    }
    if local.screenshot_bytes_used.saturating_add(byte_len) > local.screenshot_bytes {
        return Some(Err("budget"));
    }
    local.screenshot_count_used += 1;
    local.screenshot_bytes_used = local.screenshot_bytes_used.saturating_add(byte_len);
    LOCAL_BUDGET.with(|cell| *cell.borrow_mut() = Some(local));
    Some(Ok(()))
}

#[cfg(test)]
fn reserve_refmap_local(byte_len: u64) -> Option<Result<(), &'static str>> {
    let mut local = LOCAL_BUDGET.with(|cell| *cell.borrow())?;
    if local.refmap_bytes_used.saturating_add(byte_len) > local.refmap_bytes {
        return Some(Err("budget"));
    }
    local.refmap_bytes_used = local.refmap_bytes_used.saturating_add(byte_len);
    LOCAL_BUDGET.with(|cell| *cell.borrow_mut() = Some(local));
    Some(Ok(()))
}

fn reserve_screenshot(byte_len: u64) -> Result<(), &'static str> {
    #[cfg(test)]
    if let Some(result) = reserve_screenshot_local(byte_len) {
        return result;
    }
    let current_count = SCREENSHOT_COUNT_USED.load(Ordering::Relaxed);
    if current_count >= SCREENSHOT_COUNT_LIMIT.load(Ordering::Relaxed) {
        return Err("count_budget");
    }
    let current_bytes = SCREENSHOT_BYTES_USED.load(Ordering::Relaxed);
    if current_bytes.saturating_add(byte_len) > SCREENSHOT_BYTE_LIMIT.load(Ordering::Relaxed) {
        return Err("budget");
    }
    SCREENSHOT_COUNT_USED.fetch_add(1, Ordering::Relaxed);
    SCREENSHOT_BYTES_USED.fetch_add(byte_len, Ordering::Relaxed);
    Ok(())
}

fn reserve_refmap(byte_len: u64) -> Result<(), &'static str> {
    #[cfg(test)]
    if let Some(result) = reserve_refmap_local(byte_len) {
        return result;
    }
    let current = REFMAP_BYTES_USED.load(Ordering::Relaxed);
    if current.saturating_add(byte_len) > REFMAP_BYTE_LIMIT.load(Ordering::Relaxed) {
        return Err("budget");
    }
    REFMAP_BYTES_USED.fetch_add(byte_len, Ordering::Relaxed);
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArtifactOutcome {
    Captured(String),
    Skipped(String),
}

fn process_start_ms() -> u64 {
    static START_MS: OnceLock<u64> = OnceLock::new();
    *START_MS.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    })
}

fn artifacts_enabled(context: &CommandContext) -> bool {
    context.trace_enabled() && context.artifacts_full()
}

fn session_trace_dir(context: &CommandContext) -> Option<PathBuf> {
    let session_id = context.session_id()?;
    let store = RefStore::for_session(Some(session_id)).ok()?;
    Some(store.trace_dir())
}

fn screens_dir(trace_dir: &Path) -> PathBuf {
    trace_dir.join("screens")
}

fn refmaps_dir(trace_dir: &Path) -> PathBuf {
    trace_dir.join("refmaps")
}

fn is_symlink_path(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|meta| meta.file_type().is_symlink())
        .unwrap_or(false)
}

fn relative_to_trace(trace_dir: &Path, path: &Path) -> String {
    path.strip_prefix(trace_dir)
        .map(|rel| rel.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|_| path.to_string_lossy().into_owned())
}

pub(crate) fn capture_action_screenshot(
    context: &CommandContext,
    adapter: &dyn PlatformAdapter,
    pid: i32,
    phase: &str,
) -> ArtifactOutcome {
    if !artifacts_enabled(context) {
        return ArtifactOutcome::Skipped("disabled".into());
    }
    let Some(trace_dir) = session_trace_dir(context) else {
        return ArtifactOutcome::Skipped("no_session".into());
    };
    let screens = screens_dir(&trace_dir);
    if screens.exists() && is_symlink_path(&screens) {
        return ArtifactOutcome::Skipped("symlinked_dir".into());
    }
    if let Err(err) = ensure_trace_dir(&screens) {
        return ArtifactOutcome::Skipped(format!("dir: {err}"));
    }

    let buf = match adapter.screenshot(ScreenshotTarget::Window(pid)) {
        Ok(buf) => buf,
        Err(err) => {
            return ArtifactOutcome::Skipped(format!("adapter: {}", err.code.as_str()));
        }
    };
    let byte_len = buf.data.len() as u64;
    if let Err(reason) = reserve_screenshot(byte_len) {
        return ArtifactOutcome::Skipped(reason.into());
    }

    let seq = CAPTURE_SEQ.fetch_add(1, Ordering::Relaxed);
    let filename = format!("{}-{}-{}-{}.png", pid, process_start_ms(), seq, phase);
    let path = screens.join(&filename);
    if write_private_file(&path, &buf.data).is_err() {
        return ArtifactOutcome::Skipped("write_failed".into());
    }
    ArtifactOutcome::Captured(relative_to_trace(&trace_dir, &path))
}

pub(crate) fn copy_refmap_if_full(
    context: &CommandContext,
    store: &RefStore,
    snapshot_id: &str,
) -> Result<(), AppError> {
    if !artifacts_enabled(context) {
        return Ok(());
    }
    let trace_dir = store.trace_dir();
    let refmaps = refmaps_dir(&trace_dir);
    if refmaps.exists() && is_symlink_path(&refmaps) {
        return Ok(());
    }
    ensure_trace_dir(&refmaps)?;
    let dest = refmaps.join(format!("{snapshot_id}.json"));
    if dest.is_file() {
        return Ok(());
    }
    let refmap = store.load(Some(snapshot_id))?;
    let json = refmap.serialize_with_size_check()?;
    let byte_len = json.len() as u64;
    if reserve_refmap(byte_len).is_err() {
        return Ok(());
    }
    let _ = write_private_file(&dest, json.as_bytes());
    Ok(())
}

pub(crate) fn emit_action_artifacts(
    context: &CommandContext,
    ref_id: &str,
    pre: &ArtifactOutcome,
    post: &ArtifactOutcome,
) -> Result<(), AppError> {
    if !artifacts_enabled(context) {
        return Ok(());
    }
    let same_skip = match (pre, post) {
        (ArtifactOutcome::Skipped(a), ArtifactOutcome::Skipped(b)) if a == b && a != "disabled" => {
            Some(a.as_str())
        }
        _ => None,
    };
    if let Some(reason) = same_skip {
        return context.trace(
            "action.artifacts",
            json!({ "ref": ref_id, "skipped": reason }),
        );
    }
    let mut fields = json!({ "ref": ref_id });
    match pre {
        ArtifactOutcome::Captured(path) => fields["screenshot_pre"] = json!(path),
        ArtifactOutcome::Skipped(reason) if reason != "disabled" => {
            fields["skipped_pre"] = json!(reason);
        }
        _ => {}
    }
    match post {
        ArtifactOutcome::Captured(path) => fields["screenshot_post"] = json!(path),
        ArtifactOutcome::Skipped(reason) if reason != "disabled" => {
            fields["skipped_post"] = json!(reason);
        }
        _ => {}
    }
    context.trace("action.artifacts", fields)
}

pub(crate) fn resolve_screenshot_path(trace_dir: &Path, relative: &str) -> Option<PathBuf> {
    if relative.is_empty() || relative.starts_with('/') || relative.contains('\\') {
        return None;
    }
    let path = PathBuf::from(relative);
    for component in path.components() {
        if matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        ) {
            return None;
        }
    }
    let joined = trace_dir.join(&path);
    let canonical = joined.canonicalize().ok()?;
    let trace_canonical = trace_dir.canonicalize().ok()?;
    if !canonical.starts_with(&trace_canonical) {
        return None;
    }
    if is_symlink_path(&joined) {
        return None;
    }
    Some(joined)
}

pub(crate) fn read_screenshot_for_embed(trace_dir: &Path, relative: &str) -> Option<Vec<u8>> {
    let path = resolve_screenshot_path(trace_dir, relative)?;
    std::fs::read(path).ok()
}

#[cfg(test)]
#[path = "trace_artifacts_tests.rs"]
mod tests;
