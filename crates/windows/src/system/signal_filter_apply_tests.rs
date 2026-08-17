use super::*;
use agent_desktop_core::{AppInfo, ProcessIdentity, WindowInfo, WindowState};

fn window(id: &str, app: &str, pid: u32, instance: &str) -> WindowInfo {
    WindowInfo {
        id: id.to_string(),
        title: String::new(),
        app: app.to_string(),
        pid: ProcessId::from(pid),
        process_instance: Some(instance.to_string()),
        bounds: None,
        state: WindowState {
            is_focused: false,
            minimized: None,
            visible: None,
        },
    }
}

fn app_info(name: &str, pid: u32, instance: &str) -> AppInfo {
    AppInfo {
        name: name.to_string(),
        pid: ProcessId::from(pid),
        bundle_id: None,
        process_instance: Some(instance.to_string()),
        presentation: None,
    }
}

fn inventory(windows: Vec<WindowInfo>, apps: Vec<AppInfo>) -> SignalWindowInventory {
    SignalWindowInventory {
        windows,
        apps,
        windows_complete: true,
        apps_complete: true,
        excluded_window_count: 0,
    }
}

/// Reproduces `crates/core/src/commands/wait_event.rs:158-196`'s own
/// predicate rather than paraphrasing it: for a filtered wait, every
/// observed pid and process instance must equal the expected one exactly.
fn satisfies_validate_signal_scope(
    inventory: &SignalWindowInventory,
    expected: &ProcessIdentity,
) -> bool {
    inventory
        .windows
        .iter()
        .map(|window| (window.pid, window.process_instance.as_deref()))
        .chain(
            inventory
                .apps
                .iter()
                .map(|app| (app.pid, app.process_instance.as_deref())),
        )
        .all(|(pid, instance)| pid == expected.pid && instance == Some(expected.instance.as_str()))
}

#[test]
fn a_process_filter_returns_only_the_matching_pid_when_two_entries_share_an_image_name() {
    let source = inventory(
        vec![
            window("w-1", "shared.exe", 100, "gen-a"),
            window("w-2", "shared.exe", 200, "gen-b"),
        ],
        vec![
            app_info("shared.exe", 100, "gen-a"),
            app_info("shared.exe", 200, "gen-b"),
        ],
    );
    let filter = SignalFilter {
        app: None,
        process: Some(ProcessIdentity::new(100u32, "gen-a")),
    };

    let filtered = apply_signal_filter(source, &filter);

    assert_eq!(filtered.windows.len(), 1);
    assert_eq!(filtered.windows[0].pid, ProcessId::from(100u32));
    assert_eq!(filtered.apps.len(), 1);
    assert_eq!(filtered.apps[0].pid, ProcessId::from(100u32));
}

#[test]
fn a_process_filtered_result_satisfies_cores_own_scope_validation_predicate() {
    let source = inventory(
        vec![
            window("w-1", "shared.exe", 100, "gen-a"),
            window("w-2", "shared.exe", 200, "gen-b"),
        ],
        vec![
            app_info("shared.exe", 100, "gen-a"),
            app_info("shared.exe", 200, "gen-b"),
        ],
    );
    let expected = ProcessIdentity::new(100u32, "gen-a");
    let filter = SignalFilter {
        app: None,
        process: Some(expected.clone()),
    };

    let filtered = apply_signal_filter(source, &filter);

    assert!(
        satisfies_validate_signal_scope(&filtered, &expected),
        "a process-filtered result must carry no window or app whose pid or \
         instance differs from the filter's expected process, or core's \
         validate_signal_scope aborts the wait"
    );
}

#[test]
fn an_app_only_filter_matches_case_insensitively_and_not_by_substring() {
    let source = inventory(
        vec![
            window("w-1", "notepad.exe", 100, "gen-a"),
            window("w-2", "notes.exe", 200, "gen-b"),
        ],
        vec![
            app_info("notepad.exe", 100, "gen-a"),
            app_info("notes.exe", 200, "gen-b"),
        ],
    );
    let filter = SignalFilter {
        app: Some("NOTEPAD.EXE".to_string()),
        process: None,
    };

    let filtered = apply_signal_filter(source, &filter);

    assert_eq!(filtered.windows.len(), 1);
    assert_eq!(filtered.windows[0].app, "notepad.exe");
    assert_eq!(filtered.apps.len(), 1);
    assert_eq!(filtered.apps[0].name, "notepad.exe");
}

#[test]
fn an_app_only_filter_does_not_match_a_substring_of_a_longer_name() {
    let source = inventory(
        vec![
            window("w-1", "notepad.exe", 100, "gen-a"),
            window("w-2", "notes.exe", 200, "gen-b"),
        ],
        vec![
            app_info("notepad.exe", 100, "gen-a"),
            app_info("notes.exe", 200, "gen-b"),
        ],
    );
    let filter = SignalFilter {
        app: Some("note".to_string()),
        process: None,
    };

    let filtered = apply_signal_filter(source, &filter);

    assert!(
        filtered.windows.is_empty() && filtered.apps.is_empty(),
        "'note' is a substring of both 'notepad.exe' and 'notes.exe', so a \
         substring match would wrongly admit both; the exact predicate must \
         admit neither"
    );
}

#[test]
fn an_entity_whose_process_instance_differs_while_pid_matches_is_excluded() {
    let source = inventory(
        vec![window("w-1", "shared.exe", 100, "gen-old")],
        vec![app_info("shared.exe", 100, "gen-old")],
    );
    let filter = SignalFilter {
        app: None,
        process: Some(ProcessIdentity::new(100u32, "gen-new")),
    };

    let filtered = apply_signal_filter(source, &filter);

    assert!(filtered.windows.is_empty());
    assert!(filtered.apps.is_empty());
}

#[test]
fn filtering_to_a_process_with_no_windows_returns_empty_vectors_with_completeness_intact() {
    let source = inventory(
        vec![window("w-1", "other.exe", 999, "gen-x")],
        vec![app_info("other.exe", 999, "gen-x")],
    );
    let filter = SignalFilter {
        app: None,
        process: Some(ProcessIdentity::new(100u32, "gen-a")),
    };

    let filtered = apply_signal_filter(source, &filter);

    assert!(filtered.windows.is_empty());
    assert!(filtered.apps.is_empty());
    assert!(
        filtered.windows_complete && filtered.apps_complete,
        "an empty filtered result must not read as an incomplete observation - \
         the app-terminated poll depends on this staying true"
    );
}

#[test]
fn both_set_intersects_and_a_name_mismatch_excludes_a_pid_that_would_otherwise_match() {
    let source = inventory(
        vec![window("w-1", "other.exe", 100, "gen-a")],
        vec![app_info("other.exe", 100, "gen-a")],
    );
    let filter = SignalFilter {
        app: Some("notepad.exe".to_string()),
        process: Some(ProcessIdentity::new(100u32, "gen-a")),
    };

    let filtered = apply_signal_filter(source, &filter);

    assert!(
        filtered.windows.is_empty() && filtered.apps.is_empty(),
        "a pid/instance match alone must not be enough once an app name is \
         also set and disagrees"
    );
}

#[test]
fn an_empty_filter_returns_the_full_population_unchanged() {
    let source = inventory(
        vec![
            window("w-1", "one.exe", 100, "gen-a"),
            window("w-2", "two.exe", 200, "gen-b"),
        ],
        vec![
            app_info("one.exe", 100, "gen-a"),
            app_info("two.exe", 200, "gen-b"),
        ],
    );
    let filter = SignalFilter::default();

    let filtered = apply_signal_filter(source, &filter);

    assert_eq!(filtered.windows.len(), 2);
    assert_eq!(filtered.apps.len(), 2);
}

#[test]
fn excluded_window_count_and_completeness_pass_through_the_filter_unchanged() {
    let mut source = inventory(
        vec![window("w-1", "shared.exe", 100, "gen-a")],
        vec![app_info("shared.exe", 100, "gen-a")],
    );
    source.windows_complete = false;
    source.excluded_window_count = 3;
    let filter = SignalFilter {
        app: None,
        process: Some(ProcessIdentity::new(100u32, "gen-a")),
    };

    let filtered = apply_signal_filter(source, &filter);

    assert!(!filtered.windows_complete);
    assert_eq!(filtered.excluded_window_count, 3);
}
