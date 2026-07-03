use agent_desktop_core::{
    adapter::{SnapshotSurface, WindowFilter},
    error::AdapterError,
    node::AppInfo,
    signals::{SignalBaseline, SignalFilter, SurfaceSignal},
};

#[cfg(target_os = "macos")]
pub fn supported_surfaces_impl() -> Vec<SnapshotSurface> {
    vec![
        SnapshotSurface::Window,
        SnapshotSurface::Focused,
        SnapshotSurface::Menu,
        SnapshotSurface::Menubar,
        SnapshotSurface::Sheet,
        SnapshotSurface::Popover,
        SnapshotSurface::Alert,
    ]
}

#[cfg(not(target_os = "macos"))]
pub fn supported_surfaces_impl() -> Vec<SnapshotSurface> {
    vec![SnapshotSurface::Window]
}

#[cfg(target_os = "macos")]
pub fn capture_signal_baseline_impl(filter: &SignalFilter) -> Result<SignalBaseline, AdapterError> {
    let windows = crate::system::window_list::list_windows_impl(&WindowFilter {
        focused_only: false,
        app: filter.app.clone(),
    })?;
    let apps = matching_apps(filter);
    let surfaces = surfaces_for_apps(filter, &apps);
    Ok(SignalBaseline {
        windows,
        apps,
        surfaces,
    })
}

#[cfg(not(target_os = "macos"))]
pub fn capture_signal_baseline_impl(
    _filter: &SignalFilter,
) -> Result<SignalBaseline, AdapterError> {
    Err(AdapterError::not_supported("capture_signal_baseline"))
}

#[cfg(target_os = "macos")]
fn matching_apps(filter: &SignalFilter) -> Vec<AppInfo> {
    let all = crate::system::app_list::list_apps_impl().unwrap_or_default();
    if let Some(name) = &filter.app {
        return all
            .into_iter()
            .filter(|app| app.name.eq_ignore_ascii_case(name))
            .collect();
    }
    if let Some(pid) = filter.pid {
        return all.into_iter().filter(|app| app.pid == pid).collect();
    }
    all
}

/// Surfaces (sheets, alerts, popovers, open menus) are only inspected for
/// apps the caller has named — walking every running app's accessibility
/// tree on every poll to look for a stray sheet would be prohibitively slow
/// and noisy. This mirrors `list_surfaces_for_pid`'s existing per-app scope.
#[cfg(target_os = "macos")]
fn surfaces_for_apps(filter: &SignalFilter, apps: &[AppInfo]) -> Vec<SurfaceSignal> {
    if filter.app.is_none() && filter.pid.is_none() {
        return Vec::new();
    }
    let mut surfaces = Vec::new();
    for app in apps {
        for info in crate::tree::surfaces::list_surfaces_for_pid(app.pid) {
            if let Some(kind) = map_surface_kind(&info.kind) {
                surfaces.push(SurfaceSignal {
                    kind,
                    app: app.name.clone(),
                });
            }
        }
    }
    surfaces
}

#[cfg(target_os = "macos")]
fn map_surface_kind(raw: &str) -> Option<SnapshotSurface> {
    match raw {
        "sheet" => Some(SnapshotSurface::Sheet),
        "popover" => Some(SnapshotSurface::Popover),
        "alert" => Some(SnapshotSurface::Alert),
        "menu" | "context_menu" => Some(SnapshotSurface::Menu),
        _ => None,
    }
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod tests;
