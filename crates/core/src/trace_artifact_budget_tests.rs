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
