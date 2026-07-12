use super::*;

fn app(name: &str, pid: u32) -> AppInfo {
    AppInfo {
        name: name.to_string(),
        pid: agent_desktop_core::ProcessId::new(pid),
        bundle_id: None,
        process_instance: Some(format!("instance-{pid}")),
    }
}

fn app_with_bundle(name: &str, pid: u32, bundle_id: &str) -> AppInfo {
    AppInfo {
        name: name.to_string(),
        pid: agent_desktop_core::ProcessId::new(pid),
        bundle_id: Some(bundle_id.to_string()),
        process_instance: Some(format!("instance-{pid}")),
    }
}

#[test]
fn merge_apps_does_not_duplicate_same_pid_with_different_name() {
    let mut apps = vec![app("Preview", 42)];

    merge_apps(&mut apps, vec![app("Preview Helper", 42)]).unwrap();

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].name, "Preview");
}

#[test]
fn merge_apps_adds_bundle_id_for_existing_pid() {
    let mut apps = vec![app("Preview", 42)];

    merge_apps(
        &mut apps,
        vec![app_with_bundle("Preview Helper", 42, "com.apple.Preview")],
    )
    .unwrap();

    assert_eq!(apps.len(), 1);
    assert_eq!(apps[0].bundle_id.as_deref(), Some("com.apple.Preview"));
}

#[test]
fn merge_apps_keeps_distinct_pids_with_same_name() {
    let mut apps = vec![app("Terminal", 10)];

    merge_apps(&mut apps, vec![app("Terminal", 11)]).unwrap();

    assert_eq!(apps.len(), 2);
    assert_eq!(apps[1].pid, 11);
}

#[test]
fn matching_pids_returns_all_exact_name_instances() {
    let apps = vec![
        app("Terminal", 3),
        app("Terminal Helper", 4),
        app("terminal", 2),
        app("Terminal", 3),
    ];

    assert_eq!(matching_pids(&apps, "Terminal"), vec![2, 3]);
}

#[test]
fn merge_apps_rejects_pid_reuse_between_sources() {
    let mut apps = vec![app("Old App", 42)];
    let mut replacement = app("Replacement App", 42);
    replacement.process_instance = Some("replacement-generation".into());

    let error = merge_apps(&mut apps, vec![replacement]).expect_err("PID reuse must fail closed");

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(error.details.unwrap()["kind"], "inventory_identity_race");
    assert_eq!(apps[0].name, "Old App");
}

#[test]
fn signal_inventory_rejects_even_one_missing_complete_source() {
    let error = complete_apps_from_sources(Ok(vec![app("Finder", 10)]), Err(source_error("ps")))
        .unwrap_err();

    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    let details = error.details.unwrap();
    assert_eq!(details["complete"], false);
    assert_eq!(details["failures"].as_array().unwrap().len(), 1);
}

#[test]
fn signal_inventory_accepts_complete_successful_empty_sources() {
    let apps = complete_apps_from_sources(Ok(Vec::new()), Ok(Vec::new())).unwrap();

    assert!(apps.is_empty());
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

fn source_error(source: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        format!("{source} inventory failed"),
    )
}
