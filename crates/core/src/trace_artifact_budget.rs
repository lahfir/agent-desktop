use crate::{
    private_file::read_private_bounded, refs::write_private_file, refs_lock::RefStoreLock,
};
use std::path::Path;

const SCREENSHOT_BYTE_BUDGET: u64 = 128 * 1024 * 1024;
const SCREENSHOT_COUNT_BUDGET: u32 = 200;
const REFMAP_BYTE_BUDGET: u64 = 64 * 1024 * 1024;
const USAGE_LEDGER_FILE: &str = ".artifact-usage.json";
const USAGE_LEDGER_MAX_BYTES: u64 = 256;
const ARTIFACT_LOCK_FILE: &str = ".artifact-budget.lock";

#[derive(Clone, Copy)]
struct ArtifactLimits {
    screenshot_bytes: u64,
    screenshot_count: u32,
    refmap_bytes: u64,
}

#[cfg(test)]
thread_local! {
    static TEST_LIMITS: std::cell::Cell<Option<ArtifactLimits>> = const { std::cell::Cell::new(None) };
    static TEST_SCAN_COUNT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
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

#[cfg(test)]
fn reset_test_scan_count() {
    TEST_SCAN_COUNT.with(|count| count.set(0));
}

#[cfg(test)]
fn test_scan_count() -> u32 {
    TEST_SCAN_COUNT.with(std::cell::Cell::get)
}

pub(crate) fn write_screenshot(
    trace_dir: &Path,
    path: &Path,
    bytes: &[u8],
    deadline: crate::Deadline,
) -> Result<(), &'static str> {
    let _lock = artifact_lock_with_deadline(trace_dir, deadline)?;
    let limits = limits();
    let usage = load_usage(trace_dir)?;
    if usage.1 >= limits.screenshot_count {
        return Err("count_budget");
    }
    if usage.0.saturating_add(bytes.len() as u64) > limits.screenshot_bytes {
        return Err("budget");
    }
    let reserved = (
        usage.0.saturating_add(bytes.len() as u64),
        usage.1.saturating_add(1),
        usage.2,
    );
    persist_usage(trace_dir, reserved)?;
    if write_private_file(path, bytes).is_err() {
        rollback_reservation_if_absent(trace_dir, path, usage);
        return Err("write_failed");
    }
    Ok(())
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
    let usage = load_usage(trace_dir)?;
    if usage.2.saturating_add(bytes.len() as u64) > limits().refmap_bytes {
        return Err("budget");
    }
    let reserved = (usage.0, usage.1, usage.2.saturating_add(bytes.len() as u64));
    persist_usage(trace_dir, reserved)?;
    if write_private_file(path, bytes).is_err() {
        rollback_reservation_if_absent(trace_dir, path, usage);
        return Err("write_failed");
    }
    Ok(())
}

fn artifact_lock(trace_dir: &Path) -> Result<RefStoreLock, &'static str> {
    RefStoreLock::acquire(&trace_dir.join(ARTIFACT_LOCK_FILE)).map_err(|_| "lock_failed")
}

fn artifact_lock_with_deadline(
    trace_dir: &Path,
    deadline: crate::Deadline,
) -> Result<RefStoreLock, &'static str> {
    RefStoreLock::acquire_with_deadline(&trace_dir.join(ARTIFACT_LOCK_FILE), deadline)
        .map_err(|_| "lock_failed")
}

fn load_usage(trace_dir: &Path) -> Result<(u64, u32, u64), &'static str> {
    let ledger = trace_dir.join(USAGE_LEDGER_FILE);
    if let Ok(bytes) = read_private_bounded(&ledger, USAGE_LEDGER_MAX_BYTES)
        && let Some(usage) = decode_usage(&bytes)
    {
        return Ok(usage);
    }
    let (screenshot_bytes, screenshot_count) = directory_usage(&trace_dir.join("screens"))?;
    let (refmap_bytes, _) = directory_usage(&trace_dir.join("refmaps"))?;
    let usage = (screenshot_bytes, screenshot_count, refmap_bytes);
    persist_usage(trace_dir, usage)?;
    Ok(usage)
}

fn decode_usage(bytes: &[u8]) -> Option<(u64, u32, u64)> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    Some((
        value.get("screenshot_bytes")?.as_u64()?,
        u32::try_from(value.get("screenshot_count")?.as_u64()?).ok()?,
        value.get("refmap_bytes")?.as_u64()?,
    ))
}

fn persist_usage(trace_dir: &Path, usage: (u64, u32, u64)) -> Result<(), &'static str> {
    let bytes = serde_json::to_vec(&serde_json::json!({
        "screenshot_bytes": usage.0,
        "screenshot_count": usage.1,
        "refmap_bytes": usage.2,
    }))
    .map_err(|_| "ledger_failed")?;
    write_private_file(&trace_dir.join(USAGE_LEDGER_FILE), &bytes).map_err(|_| "ledger_failed")
}

fn rollback_reservation_if_absent(trace_dir: &Path, path: &Path, usage: (u64, u32, u64)) {
    let artifact_may_exist = path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_file() || metadata.file_type().is_symlink());
    if !artifact_may_exist {
        let _ = persist_usage(trace_dir, usage);
    }
}

fn directory_usage(dir: &Path) -> Result<(u64, u32), &'static str> {
    #[cfg(test)]
    TEST_SCAN_COUNT.with(|count| count.set(count.get().saturating_add(1)));
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

    fn prepare_trace(name: &str) -> std::path::PathBuf {
        let trace = temp_dir(name);
        let _ = std::fs::remove_dir_all(&trace);
        crate::trace::ensure_trace_dir(&trace.join("screens")).unwrap();
        crate::trace::ensure_trace_dir(&trace.join("refmaps")).unwrap();
        trace
    }

    fn test_deadline() -> crate::Deadline {
        crate::Deadline::standard().unwrap()
    }

    #[test]
    fn artifact_writes_scan_once_then_use_the_private_ledger() {
        let trace = prepare_trace("scan-once");
        reset_test_scan_count();
        set_test_limits(100, 10, 100);

        write_screenshot(
            &trace,
            &trace.join("screens/a.png"),
            &[1, 2],
            test_deadline(),
        )
        .unwrap();
        assert_eq!(test_scan_count(), 2);
        write_screenshot(&trace, &trace.join("screens/b.png"), &[3], test_deadline()).unwrap();
        write_refmap_if_absent(&trace, &trace.join("refmaps/a.json"), &[4, 5]).unwrap();
        write_refmap_if_absent(&trace, &trace.join("refmaps/b.json"), &[6]).unwrap();

        assert_eq!(test_scan_count(), 2);
        let ledger =
            read_private_bounded(&trace.join(USAGE_LEDGER_FILE), USAGE_LEDGER_MAX_BYTES).unwrap();
        assert_eq!(decode_usage(&ledger), Some((3, 2, 3)));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(trace.join(USAGE_LEDGER_FILE))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        clear_test_limits();
        std::fs::remove_dir_all(trace).unwrap();
    }

    #[test]
    fn missing_and_corrupt_ledgers_are_repaired_from_artifacts() {
        let trace = prepare_trace("repair");
        set_test_limits(100, 10, 100);
        write_screenshot(&trace, &trace.join("screens/a.png"), &[1], test_deadline()).unwrap();

        std::fs::remove_file(trace.join(USAGE_LEDGER_FILE)).unwrap();
        reset_test_scan_count();
        write_screenshot(&trace, &trace.join("screens/b.png"), &[2], test_deadline()).unwrap();
        assert_eq!(test_scan_count(), 2);
        write_private_file(&trace.join(USAGE_LEDGER_FILE), b"invalid").unwrap();
        reset_test_scan_count();
        write_refmap_if_absent(&trace, &trace.join("refmaps/a.json"), &[3]).unwrap();

        assert_eq!(test_scan_count(), 2);
        let ledger =
            read_private_bounded(&trace.join(USAGE_LEDGER_FILE), USAGE_LEDGER_MAX_BYTES).unwrap();
        assert_eq!(decode_usage(&ledger), Some((2, 2, 1)));
        clear_test_limits();
        std::fs::remove_dir_all(trace).unwrap();
    }

    #[test]
    fn failed_artifact_write_rolls_back_its_reservation() {
        let trace = prepare_trace("rollback");
        let blocked = trace.join("screens/blocked.png");
        std::fs::create_dir(&blocked).unwrap();
        set_test_limits(1, 1, 100);

        assert_eq!(
            write_screenshot(&trace, &blocked, &[1], test_deadline()),
            Err("write_failed")
        );
        std::fs::remove_dir(&blocked).unwrap();
        assert!(
            write_screenshot(&trace, &trace.join("screens/ok.png"), &[1], test_deadline(),).is_ok()
        );

        clear_test_limits();
        std::fs::remove_dir_all(trace).unwrap();
    }

    #[test]
    fn reserved_usage_without_an_artifact_remains_fail_safe() {
        let trace = prepare_trace("reserved");
        persist_usage(&trace, (1, 1, 0)).unwrap();
        set_test_limits(1, 1, 100);

        assert_eq!(
            write_screenshot(
                &trace,
                &trace.join("screens/next.png"),
                &[1],
                test_deadline(),
            ),
            Err("count_budget")
        );

        clear_test_limits();
        std::fs::remove_dir_all(trace).unwrap();
    }

    #[test]
    fn ambiguous_write_failure_with_a_file_keeps_the_reservation() {
        let trace = prepare_trace("ambiguous-failure");
        let path = trace.join("screens/maybe-written.png");
        write_private_file(&path, &[1]).unwrap();
        persist_usage(&trace, (1, 1, 0)).unwrap();

        rollback_reservation_if_absent(&trace, &path, (0, 0, 0));

        let ledger =
            read_private_bounded(&trace.join(USAGE_LEDGER_FILE), USAGE_LEDGER_MAX_BYTES).unwrap();
        assert_eq!(decode_usage(&ledger), Some((1, 1, 0)));
        std::fs::remove_dir_all(trace).unwrap();
    }

    #[test]
    fn preexisting_files_consume_the_persisted_session_budget() {
        let trace = temp_dir("persisted");
        let screens = trace.join("screens");
        let _ = std::fs::remove_dir_all(&trace);
        crate::trace::ensure_trace_dir(&screens).unwrap();
        std::fs::write(screens.join("existing.png"), [1]).unwrap();
        set_test_limits(100, 1, 100);

        let result = write_screenshot(&trace, &screens.join("next.png"), &[2], test_deadline());

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

        assert!(
            write_screenshot(&first, &first.join("screens/a.png"), &[1], test_deadline(),).is_ok()
        );
        assert!(
            write_screenshot(
                &second,
                &second.join("screens/a.png"),
                &[1],
                test_deadline(),
            )
            .is_ok()
        );

        clear_test_limits();
        std::fs::remove_dir_all(first).unwrap();
        std::fs::remove_dir_all(second).unwrap();
    }
}
