use super::*;

fn app(name: &str, pid: i32) -> AppInfo {
    AppInfo {
        name: name.to_string(),
        pid,
        bundle_id: None,
    }
}

fn app_with_bundle(name: &str, pid: i32, bundle_id: &str) -> AppInfo {
    AppInfo {
        name: name.to_string(),
        pid,
        bundle_id: Some(bundle_id.to_string()),
    }
}

#[test]
fn merge_apps_does_not_duplicate_same_pid_with_different_name() {
    let mut apps = vec![app("Preview", 42)];

    merge_apps(&mut apps, vec![app("Preview Helper", 42)]);

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "Preview");
}

#[test]
fn merge_apps_adds_bundle_id_for_existing_pid() {
    let mut apps = vec![app("Preview", 42)];

    merge_apps(
        &mut apps,
        vec![app_with_bundle("Preview Helper", 42, "com.apple.Preview")],
    );

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].bundle_id.as_deref(), Some("com.apple.Preview"));
}

#[test]
fn merge_apps_keeps_distinct_pids_with_same_name() {
    let mut apps = vec![app("Terminal", 10)];

    merge_apps(&mut apps, vec![app("Terminal", 11)]);

    assert_eq!(apps.len(), 2);
    assert_eq!(apps[1].pid, 11);
}

#[test]
fn find_app_in_apps_prefers_exact_case_insensitive_match() {
    let apps = vec![app("Finder Helper", 10), app("Finder", 11)];

    assert_eq!(
        find_app_in_apps(&apps, "finder").map(|app| app.pid),
        Some(11)
    );
}

#[test]
fn find_app_in_apps_rejects_contains_match() {
    let apps = vec![app("Mail Helper", 10), app("Docker Desktop", 11)];

    assert!(find_app_in_apps(&apps, "Mail").is_none());
    assert!(find_app_in_apps(&apps, "Docker").is_none());
}

#[test]
fn find_app_with_process_fallback_uses_process_entries_after_primary_miss() {
    let primary = vec![app("Finder", 10)];

    assert_eq!(
        find_app_with_process_fallback(&primary, || vec![app("Mail", 11)], "Mail")
            .map(|app| app.pid),
        Some(11)
    );
}

#[test]
fn find_app_with_process_fallback_prefers_primary_entries() {
    let primary = vec![app("Mail", 10)];
    let mut process_called = false;

    assert_eq!(
        find_app_with_process_fallback(
            &primary,
            || {
                process_called = true;
                vec![app("Mail", 11)]
            },
            "Mail"
        )
        .map(|app| app.pid),
        Some(10)
    );
    assert!(!process_called);
}

#[test]
fn find_app_with_process_fallback_does_not_cross_match_helpers() {
    let primary = Vec::new();

    assert!(
        find_app_with_process_fallback(&primary, || vec![app("Mail Helper", 11)], "Mail").is_none()
    );
}

#[test]
fn list_apps_from_sources_includes_process_apps_when_primary_has_entries() {
    let apps = list_apps_from_sources(vec![app("Finder", 10)], Vec::new(), vec![app("Mail", 11)]);

    assert_eq!(
        apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
        vec!["Finder", "Mail"]
    );
}

#[test]
fn app_for_name_from_sources_uses_visible_entries_without_process_lookup() {
    let mut process_called = false;
    let app = app_for_name_from_sources("Finder", Vec::new(), &[app("Finder", 10)], || {
        process_called = true;
        Vec::new()
    });

    assert_eq!(app.map(|app| app.pid), Some(10));
    assert!(!process_called);
}

#[test]
fn apps_from_windows_deduplicates_visible_window_apps() {
    let apps = apps_from_windows(vec![
        window("Finder", 10, "Documents", 1),
        window("Finder", 10, "Downloads", 2),
        window("Mail", 11, "Inbox", 3),
    ]);

    assert_eq!(
        apps.iter().map(|app| app.name.as_str()).collect::<Vec<_>>(),
        vec!["Finder", "Mail"]
    );
}

#[test]
fn sort_apps_orders_by_name_then_pid() {
    let mut apps = vec![app("Terminal", 3), app("Finder", 2), app("Finder", 1)];

    sort_apps(&mut apps);

    assert_eq!(
        apps.iter().map(|app| app.pid).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
}

fn window(app_name: &str, pid: i32, title: &str, window_number: i64) -> WindowInfo {
    WindowInfo {
        id: format!("w-{window_number}"),
        title: title.to_string(),
        app: app_name.to_string(),
        pid,
        bounds: None,
        is_focused: false,
    }
}
