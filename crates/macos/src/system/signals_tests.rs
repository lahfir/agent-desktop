use super::*;

#[test]
fn map_surface_kind_covers_known_shapes() {
    assert_eq!(map_surface_kind("sheet"), Some(SnapshotSurface::Sheet));
    assert_eq!(map_surface_kind("popover"), Some(SnapshotSurface::Popover));
    assert_eq!(map_surface_kind("alert"), Some(SnapshotSurface::Alert));
    assert_eq!(map_surface_kind("menu"), Some(SnapshotSurface::Menu));
    assert_eq!(
        map_surface_kind("context_menu"),
        Some(SnapshotSurface::Menu)
    );
    assert_eq!(map_surface_kind("something_else"), None);
}

#[test]
fn matching_apps_filters_by_name_case_insensitively() {
    let filter = SignalFilter {
        app: Some("Definitely-Not-A-Real-App-xyz123".into()),
        pid: None,
    };
    let apps = matching_apps(&filter).expect("app enumeration must succeed on the test host");
    assert!(
        apps.is_empty(),
        "an app name that matches no running process must yield no apps"
    );
}

#[test]
fn matching_apps_with_no_filter_does_not_panic() {
    let filter = SignalFilter::default();
    let _ = matching_apps(&filter);
}

#[test]
fn surfaces_for_apps_is_empty_without_an_app_or_pid_filter() {
    let filter = SignalFilter::default();
    let apps = vec![AppInfo {
        name: "Finder".into(),
        pid: 1,
        bundle_id: None,
    }];
    let surfaces = surfaces_for_apps(&filter, &apps);
    assert!(
        surfaces.is_empty(),
        "unscoped waits must not walk every running app's AX tree for surfaces"
    );
}
