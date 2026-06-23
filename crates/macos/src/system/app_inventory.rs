use agent_desktop_core::{
    adapter::WindowFilter,
    node::{AppInfo, WindowInfo},
};

use crate::system::{process_apps, window_inventory, workspace_apps};

pub(crate) fn list_apps() -> Vec<AppInfo> {
    let visible = window_inventory::visible_apps();
    let workspace = workspace_apps::list_apps();
    let process = process_apps::list_apps();
    tracing::debug!(
        workspace_count = workspace.len(),
        visible_count = visible.len(),
        process_count = process.len(),
        "system: app inventory sources"
    );
    let mut apps = list_apps_from_sources(workspace, visible, process);
    if apps.is_empty() {
        let fallback = window_inventory::visible_apps();
        tracing::debug!(
            fallback_count = fallback.len(),
            "system: app inventory visible-window fallback"
        );
        merge_apps(&mut apps, fallback);
    }
    sort_apps(&mut apps);
    apps
}

pub(crate) fn list_windows(filter: &WindowFilter) -> Vec<WindowInfo> {
    window_inventory::list_windows(filter, |app_name, visible_apps| {
        app_for_name_from_sources(
            app_name,
            workspace_apps::list_apps(),
            visible_apps,
            process_apps::list_apps,
        )
    })
}

pub(crate) fn pid_for_app_name(app_name: &str) -> Option<i32> {
    app_for_name(app_name).map(|app| app.pid)
}

pub(crate) fn pids_for_app_name(app_name: &str) -> Vec<i32> {
    matching_pids(&list_apps(), app_name)
}

pub(crate) fn app_for_name(app_name: &str) -> Option<AppInfo> {
    app_for_name_from_sources(
        app_name,
        workspace_apps::list_apps(),
        &window_inventory::visible_apps(),
        process_apps::list_apps,
    )
}

fn app_for_name_from_sources(
    app_name: &str,
    workspace: Vec<AppInfo>,
    visible: &[AppInfo],
    process: impl FnOnce() -> Vec<AppInfo>,
) -> Option<AppInfo> {
    let primary = merge_primary_sources(workspace, visible.to_vec());
    find_app_with_process_fallback(&primary, process, app_name)
}

fn list_apps_from_sources(
    workspace: Vec<AppInfo>,
    visible: Vec<AppInfo>,
    process: Vec<AppInfo>,
) -> Vec<AppInfo> {
    let mut apps = merge_primary_sources(workspace, visible);
    merge_apps(&mut apps, process);
    apps
}

fn merge_primary_sources(workspace: Vec<AppInfo>, visible: Vec<AppInfo>) -> Vec<AppInfo> {
    let mut apps = workspace;
    merge_apps(&mut apps, visible);
    apps
}

fn find_app_with_process_fallback(
    primary: &[AppInfo],
    process: impl FnOnce() -> Vec<AppInfo>,
    app_name: &str,
) -> Option<AppInfo> {
    find_app_in_apps(primary, app_name).or_else(|| find_app_in_apps(&process(), app_name))
}

fn merge_apps(apps: &mut Vec<AppInfo>, incoming: Vec<AppInfo>) {
    let mut seen_pids = apps
        .iter()
        .map(|app| app.pid)
        .collect::<rustc_hash::FxHashSet<_>>();

    for app in incoming {
        if seen_pids.insert(app.pid) {
            apps.push(app);
        } else if let Some(existing) = apps.iter_mut().find(|existing| existing.pid == app.pid) {
            if existing.bundle_id.is_none() {
                existing.bundle_id = app.bundle_id;
            }
        }
    }
}

fn sort_apps(apps: &mut [AppInfo]) {
    apps.sort_by(|a, b| {
        a.name
            .to_ascii_lowercase()
            .cmp(&b.name.to_ascii_lowercase())
            .then_with(|| a.pid.cmp(&b.pid))
    });
}

fn find_app_in_apps(apps: &[AppInfo], app_name: &str) -> Option<AppInfo> {
    apps.iter()
        .find(|app| app.name.eq_ignore_ascii_case(app_name))
        .cloned()
}

fn matching_pids(apps: &[AppInfo], app_name: &str) -> Vec<i32> {
    let mut pids = apps
        .iter()
        .filter(|app| app.name.eq_ignore_ascii_case(app_name))
        .map(|app| app.pid)
        .collect::<Vec<_>>();
    pids.sort_unstable();
    pids.dedup();
    pids
}

#[cfg(test)]
#[path = "app_inventory_tests.rs"]
mod tests;
