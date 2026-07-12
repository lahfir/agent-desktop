use crate::{refs::write_private_file, refs_lock::RefStoreLock};
use std::path::Path;

const SCREENSHOT_BYTE_BUDGET: u64 = 128 * 1024 * 1024;
const SCREENSHOT_COUNT_BUDGET: u32 = 200;
const REFMAP_BYTE_BUDGET: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy)]
struct ArtifactLimits {
    screenshot_bytes: u64,
    screenshot_count: u32,
    refmap_bytes: u64,
}

#[cfg(test)]
thread_local! {
    static TEST_LIMITS: std::cell::Cell<Option<ArtifactLimits>> = const { std::cell::Cell::new(None) };
}

#[cfg(test)]
pub(crate) fn set_test_limits(screenshot_bytes: u64, screenshot_count: u32, refmap_bytes: u64) {
    TEST_LIMITS.with(|limits| {
        limits.set(Some(ArtifactLimits {
            screenshot_bytes,
            screenshot_count,
            refmap_bytes,
        }));
    });
}

#[cfg(test)]
pub(crate) fn clear_test_limits() {
    TEST_LIMITS.with(|limits| limits.set(None));
}

pub(crate) fn write_screenshot(
    trace_dir: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<(), &'static str> {
    let _lock = artifact_lock(trace_dir)?;
    let limits = limits();
    let (used_bytes, used_count) = directory_usage(path.parent().ok_or("dir")?)?;
    if used_count >= limits.screenshot_count {
        return Err("count_budget");
    }
    if used_bytes.saturating_add(bytes.len() as u64) > limits.screenshot_bytes {
        return Err("budget");
    }
    write_private_file(path, bytes).map_err(|_| "write_failed")
}

pub(crate) fn write_refmap_if_absent(
    trace_dir: &Path,
    path: &Path,
    bytes: &[u8],
) -> Result<(), &'static str> {
    let _lock = artifact_lock(trace_dir)?;
    if path.is_file() {
        return Ok(());
    }
    let (used_bytes, _) = directory_usage(path.parent().ok_or("dir")?)?;
    if used_bytes.saturating_add(bytes.len() as u64) > limits().refmap_bytes {
        return Err("budget");
    }
    write_private_file(path, bytes).map_err(|_| "write_failed")
}

fn artifact_lock(trace_dir: &Path) -> Result<RefStoreLock, &'static str> {
    RefStoreLock::acquire(&trace_dir.join("artifact-budget.lock")).map_err(|_| "lock_failed")
}

fn directory_usage(dir: &Path) -> Result<(u64, u32), &'static str> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((0, 0)),
        Err(_) => return Err("scan_failed"),
    };
    let mut bytes = 0_u64;
    let mut count = 0_u32;
    for entry in entries {
        let entry = entry.map_err(|_| "scan_failed")?;
        let metadata = entry.path().symlink_metadata().map_err(|_| "scan_failed")?;
        if metadata.file_type().is_symlink() {
            return Err("scan_failed");
        }
        if metadata.is_file() {
            bytes = bytes.saturating_add(metadata.len());
            count = count.saturating_add(1);
        }
    }
    Ok((bytes, count))
}

fn limits() -> ArtifactLimits {
    #[cfg(test)]
    if let Some(limits) = TEST_LIMITS.with(std::cell::Cell::get) {
        return limits;
    }
    ArtifactLimits {
        screenshot_bytes: SCREENSHOT_BYTE_BUDGET,
        screenshot_count: SCREENSHOT_COUNT_BUDGET,
        refmap_bytes: REFMAP_BYTE_BUDGET,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "agent-desktop-artifact-budget-{name}-{}",
            std::process::id()
        ))
    }

    #[test]
    fn preexisting_files_consume_the_persisted_session_budget() {
        let trace = temp_dir("persisted");
        let screens = trace.join("screens");
        let _ = std::fs::remove_dir_all(&trace);
        crate::trace::ensure_trace_dir(&screens).unwrap();
        std::fs::write(screens.join("existing.png"), [1]).unwrap();
        set_test_limits(100, 1, 100);

        let result = write_screenshot(&trace, &screens.join("next.png"), &[2]);

        assert_eq!(result, Err("count_budget"));
        clear_test_limits();
        std::fs::remove_dir_all(trace).unwrap();
    }

    #[test]
    fn separate_session_directories_have_independent_budgets() {
        let first = temp_dir("first");
        let second = temp_dir("second");
        let _ = std::fs::remove_dir_all(&first);
        let _ = std::fs::remove_dir_all(&second);
        crate::trace::ensure_trace_dir(&first.join("screens")).unwrap();
        crate::trace::ensure_trace_dir(&second.join("screens")).unwrap();
        set_test_limits(100, 1, 100);

        assert!(write_screenshot(&first, &first.join("screens/a.png"), &[1]).is_ok());
        assert!(write_screenshot(&second, &second.join("screens/a.png"), &[1]).is_ok());

        clear_test_limits();
        std::fs::remove_dir_all(first).unwrap();
        std::fs::remove_dir_all(second).unwrap();
    }
}
