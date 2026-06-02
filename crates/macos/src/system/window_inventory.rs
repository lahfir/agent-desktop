use agent_desktop_core::{
    adapter::WindowFilter,
    node::{AppInfo, WindowInfo},
};
use std::time::Duration;

use crate::system::cg_window;

pub(crate) fn visible_apps() -> Vec<AppInfo> {
    apps_from_window_records(&cg_window::visible_window_records())
}

fn apps_from_window_records(records: &[cg_window::WindowRecord]) -> Vec<AppInfo> {
    let mut seen_pids = std::collections::HashSet::new();
    let mut apps = Vec::new();

    for record in records {
        if !seen_pids.insert(record.pid) {
            continue;
        }
        apps.push(AppInfo {
            name: record.app_name.clone(),
            pid: record.pid,
            bundle_id: None,
        });
    }

    apps
}

pub(crate) fn list_windows(
    filter: &WindowFilter,
    app_for_name: impl FnMut(&str, &[AppInfo]) -> Option<AppInfo>,
) -> Vec<WindowInfo> {
    list_windows_with_sources(
        filter,
        app_for_name,
        cg_window::visible_window_records,
        ax_window_for_app,
        std::thread::sleep,
    )
}

fn list_windows_with_sources(
    filter: &WindowFilter,
    mut app_for_name: impl FnMut(&str, &[AppInfo]) -> Option<AppInfo>,
    mut visible_records: impl FnMut() -> Vec<cg_window::WindowRecord>,
    mut ax_window_for_app: impl FnMut(&AppInfo) -> Option<WindowInfo>,
    mut sleep: impl FnMut(Duration),
) -> Vec<WindowInfo> {
    let mut app = None;
    let mut app_loaded = false;

    for attempt in 0..3 {
        let records = visible_records();
        let windows = visible_windows_from_records(filter, &records);
        if !windows.is_empty() {
            return windows;
        }

        if let Some(app_name) = filter.app.as_deref() {
            if !app_loaded {
                let visible_apps = apps_from_window_records(&records);
                app = app_for_name(app_name, &visible_apps);
                app_loaded = true;
            }
            if let Some(app) = app.as_ref() {
                if let Some(window) = ax_window_for_app(app) {
                    if !filter.focused_only || window.is_focused {
                        return vec![window];
                    }
                }
            }
        }

        if attempt == 2 || !should_retry_empty(filter, app.as_ref()) {
            break;
        }

        sleep(Duration::from_millis(50));
    }

    Vec::new()
}

fn visible_windows_from_records(
    filter: &WindowFilter,
    records: &[cg_window::WindowRecord],
) -> Vec<WindowInfo> {
    let app_filter = filter.app.as_deref().unwrap_or("").to_ascii_lowercase();
    let candidates = records
        .iter()
        .filter(|record| matches_app_filter(&record.app_name, &app_filter))
        .cloned()
        .collect();

    windows_from_records(candidates, filter.focused_only)
}

fn windows_from_records(
    records: Vec<cg_window::WindowRecord>,
    focused_only: bool,
) -> Vec<WindowInfo> {
    windows_from_records_with_focus(records, focused_only, focused_window_identity)
}

fn windows_from_records_with_focus(
    records: Vec<cg_window::WindowRecord>,
    focused_only: bool,
    mut focused_identity: impl FnMut(i32) -> FocusedWindowIdentity,
) -> Vec<WindowInfo> {
    let candidates: Vec<_> = records
        .into_iter()
        .map(|record| {
            let title = record.title.unwrap_or_else(|| record.app_name.clone());
            (record.app_name, title, record.pid, record.window_number)
        })
        .collect();
    let mut title_counts = std::collections::HashMap::new();
    for (_, title, pid, _) in &candidates {
        *title_counts.entry((*pid, title.clone())).or_insert(0) += 1;
    }

    let mut focus_cache = std::collections::HashMap::new();
    let mut windows = Vec::new();
    let mut focused_seen = false;

    for (app_name, title, pid, window_number) in candidates {
        let title_count = title_counts
            .get(&(pid, title.clone()))
            .copied()
            .unwrap_or(0);
        let identity = focus_cache
            .entry(pid)
            .or_insert_with(|| focused_identity(pid));
        let is_focused =
            !focused_seen && matches_focused_window(&title, window_number, identity, title_count);
        if focused_only && !is_focused {
            continue;
        }
        focused_seen |= is_focused;

        windows.push(WindowInfo {
            id: format!("w-{window_number}"),
            title,
            app: app_name,
            pid,
            bounds: None,
            is_focused,
        });
    }

    windows
}

fn matches_app_filter(app_name: &str, app_filter: &str) -> bool {
    app_filter.is_empty() || app_name.eq_ignore_ascii_case(app_filter)
}

fn should_retry_empty(filter: &WindowFilter, app: Option<&AppInfo>) -> bool {
    filter.app.is_none() || app.is_some()
}

fn ax_window_for_app(app_info: &AppInfo) -> Option<WindowInfo> {
    let app = crate::tree::element_for_pid(app_info.pid);
    let window = focused_window_element(&app)
        .or_else(|| crate::tree::copy_element_attr(&app, "AXMainWindow"))
        .or_else(|| {
            crate::tree::copy_ax_array(&app, "AXWindows")
                .and_then(|windows| windows.into_iter().next())
        })?;
    if crate::tree::copy_string_attr(&window, "AXRole").as_deref() != Some("AXWindow") {
        return None;
    }
    let title =
        crate::tree::copy_string_attr(&window, "AXTitle").unwrap_or_else(|| app_info.name.clone());
    let window_number = crate::tree::copy_i64_attr(&window, "AXWindowNumber").unwrap_or(0);
    let is_focused = crate::tree::copy_bool_attr(&app, "AXFrontmost") == Some(true);
    Some(ax_window_info(app_info, title, window_number, is_focused))
}

fn ax_window_info(
    app_info: &AppInfo,
    title: String,
    window_number: i64,
    is_focused: bool,
) -> WindowInfo {
    WindowInfo {
        id: format!("w-{window_number}"),
        title,
        app: app_info.name.clone(),
        pid: app_info.pid,
        bounds: None,
        is_focused,
    }
}

type FocusedWindowIdentity = Option<(Option<String>, Option<i64>)>;

fn focused_window_identity(pid: i32) -> FocusedWindowIdentity {
    let app = crate::tree::element_for_pid(pid);
    if crate::tree::copy_bool_attr(&app, "AXFrontmost") != Some(true) {
        return None;
    }
    let window = focused_window_element(&app)?;
    Some((
        crate::tree::copy_string_attr(&window, "AXTitle"),
        crate::tree::copy_i64_attr(&window, "AXWindowNumber"),
    ))
}

fn matches_focused_window(
    title: &str,
    window_number: i64,
    identity: &FocusedWindowIdentity,
    same_title_count: usize,
) -> bool {
    let Some((focused_title, focused_number)) = identity else {
        return false;
    };
    if let Some(number) = focused_number {
        return *number == window_number;
    }
    focused_title.as_deref() == Some(title) && same_title_count == 1
}

fn focused_window_element(app: &crate::tree::AXElement) -> Option<crate::tree::AXElement> {
    let focused = crate::tree::copy_element_attr(app, "AXFocusedWindow")?;
    window_ancestor(focused, 4)
}

fn window_ancestor(
    mut element: crate::tree::AXElement,
    max_depth: usize,
) -> Option<crate::tree::AXElement> {
    for _ in 0..=max_depth {
        if is_window_element(&element) {
            return Some(element);
        }
        if let Some(window) = crate::tree::copy_element_attr(&element, "AXWindow") {
            if is_window_element(&window) {
                return Some(window);
            }
        }
        element = crate::tree::copy_element_attr(&element, "AXParent")?;
    }
    None
}

fn is_window_element(element: &crate::tree::AXElement) -> bool {
    crate::tree::copy_string_attr(element, "AXRole").as_deref() == Some("AXWindow")
}

#[cfg(test)]
#[path = "window_inventory_tests.rs"]
mod tests;
