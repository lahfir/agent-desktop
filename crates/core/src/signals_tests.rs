use super::*;

fn window(id: &str, title: &str, app: &str, pid: i32, focused: bool) -> WindowInfo {
    WindowInfo {
        id: id.into(),
        title: title.into(),
        app: app.into(),
        pid,
        bounds: None,
        is_focused: focused,
    }
}

fn app(name: &str, pid: i32) -> AppInfo {
    AppInfo {
        name: name.into(),
        pid,
        bundle_id: None,
    }
}

fn baseline_with_windows(windows: Vec<WindowInfo>) -> SignalBaseline {
    SignalBaseline {
        windows,
        apps: Vec::new(),
        surfaces: Vec::new(),
    }
}

#[test]
fn new_window_id_produces_window_opened() {
    let baseline = baseline_with_windows(vec![window("w-1", "Docs", "Finder", 100, true)]);
    let current = baseline_with_windows(vec![
        window("w-1", "Docs", "Finder", 100, true),
        window("w-2", "Untitled", "TextEdit", 200, false),
    ]);

    let events = diff_signals(&baseline, &current);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::WindowOpened);
    assert_eq!(events[0].window_id.as_deref(), Some("w-2"));
    assert_eq!(events[0].title.as_deref(), Some("Untitled"));
}

#[test]
fn removed_window_id_produces_window_closed() {
    let baseline = baseline_with_windows(vec![
        window("w-1", "Docs", "Finder", 100, true),
        window("w-2", "Untitled", "TextEdit", 200, false),
    ]);
    let current = baseline_with_windows(vec![window("w-1", "Docs", "Finder", 100, true)]);

    let events = diff_signals(&baseline, &current);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::WindowClosed);
    assert_eq!(events[0].window_id.as_deref(), Some("w-2"));
}

#[test]
fn window_closed_fires_even_as_another_window_opens() {
    let baseline = baseline_with_windows(vec![
        window("w-1", "Untitled", "TextEdit", 200, true),
        window("w-9", "Finder", "Finder", 100, false),
    ]);
    let current = baseline_with_windows(vec![
        window("w-9", "Finder", "Finder", 100, false),
        window("w-42", "Preview", "Preview", 300, true),
    ]);

    let events = diff_signals(&baseline, &current);

    let closed: Vec<_> = events
        .iter()
        .filter(|e| e.kind == EventKind::WindowClosed)
        .collect();
    assert_eq!(closed.len(), 1);
    assert_eq!(closed[0].window_id.as_deref(), Some("w-1"));

    let opened: Vec<_> = events
        .iter()
        .filter(|e| e.kind == EventKind::WindowOpened)
        .collect();
    assert_eq!(opened.len(), 1);
    assert_eq!(opened[0].window_id.as_deref(), Some("w-42"));
}

#[test]
fn focus_change_produces_focus_changed_window_event() {
    let baseline = baseline_with_windows(vec![
        window("w-1", "Docs", "Finder", 100, true),
        window("w-2", "Untitled", "TextEdit", 200, false),
    ]);
    let current = baseline_with_windows(vec![
        window("w-1", "Docs", "Finder", 100, false),
        window("w-2", "Untitled", "TextEdit", 200, true),
    ]);

    let events = diff_signals(&baseline, &current);

    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::FocusChangedWindow);
    assert_eq!(events[0].window_id.as_deref(), Some("w-2"));
}

#[test]
fn focus_change_between_two_same_title_windows_reports_correct_id() {
    let baseline = baseline_with_windows(vec![
        window("w-1", "Untitled", "TextEdit", 200, true),
        window("w-2", "Untitled", "TextEdit", 200, false),
    ]);
    let current = baseline_with_windows(vec![
        window("w-1", "Untitled", "TextEdit", 200, false),
        window("w-2", "Untitled", "TextEdit", 200, true),
    ]);

    let events = diff_signals(&baseline, &current);

    let focus_events: Vec<_> = events
        .iter()
        .filter(|e| e.kind == EventKind::FocusChangedWindow)
        .collect();
    assert_eq!(focus_events.len(), 1);
    assert_eq!(
        focus_events[0].window_id.as_deref(),
        Some("w-2"),
        "focus event must key off id, not the shared title"
    );
}

#[test]
fn app_launch_and_terminate_are_detected_by_pid() {
    let baseline = SignalBaseline {
        windows: Vec::new(),
        apps: vec![app("Finder", 1)],
        surfaces: Vec::new(),
    };
    let current = SignalBaseline {
        windows: Vec::new(),
        apps: vec![app("Finder", 1), app("TextEdit", 2)],
        surfaces: Vec::new(),
    };

    let events = diff_signals(&baseline, &current);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].kind, EventKind::AppLaunched);
    assert_eq!(events[0].pid, Some(2));

    let events_back = diff_signals(&current, &baseline);
    assert_eq!(events_back.len(), 1);
    assert_eq!(events_back[0].kind, EventKind::AppTerminated);
    assert_eq!(events_back[0].pid, Some(2));
}

#[test]
fn sheet_appears_under_app_filter_produces_surface_appeared() {
    let baseline = SignalBaseline {
        windows: Vec::new(),
        apps: Vec::new(),
        surfaces: Vec::new(),
    };
    let current = SignalBaseline {
        windows: Vec::new(),
        apps: Vec::new(),
        surfaces: vec![SurfaceSignal {
            kind: SnapshotSurface::Sheet,
            app: "TextEdit".into(),
        }],
    };

    let events = diff_signals(&baseline, &current);

    assert_eq!(events.len(), 1);
    match &events[0].kind {
        EventKind::SurfaceAppeared { surface } => assert_eq!(*surface, SnapshotSurface::Sheet),
        other => panic!("expected SurfaceAppeared, got {other:?}"),
    }
    assert_eq!(events[0].app.as_deref(), Some("TextEdit"));
}

#[test]
fn surface_dismissed_fires_when_sheet_count_drops() {
    let baseline = SignalBaseline {
        windows: Vec::new(),
        apps: Vec::new(),
        surfaces: vec![SurfaceSignal {
            kind: SnapshotSurface::Sheet,
            app: "TextEdit".into(),
        }],
    };
    let current = SignalBaseline {
        windows: Vec::new(),
        apps: Vec::new(),
        surfaces: Vec::new(),
    };

    let events = diff_signals(&baseline, &current);

    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, EventKind::SurfaceDismissed { .. }));
}

#[test]
fn reorder_only_baselines_produce_zero_events() {
    let baseline = baseline_with_windows(vec![
        window("w-1", "Docs", "Finder", 100, true),
        window("w-2", "Untitled", "TextEdit", 200, false),
    ]);
    let current = baseline_with_windows(vec![
        window("w-2", "Untitled", "TextEdit", 200, false),
        window("w-1", "Docs", "Finder", 100, true),
    ]);

    let events = diff_signals(&baseline, &current);

    assert!(
        events.is_empty(),
        "reordering the same windows must not synthesize open/close/focus events, got {events:?}"
    );
}

#[test]
fn event_kind_same_variant_ignores_surface_payload() {
    let requested = EventKind::parse("surface-appeared").unwrap();
    let observed = EventKind::SurfaceAppeared {
        surface: SnapshotSurface::Alert,
    };
    assert!(requested.same_variant(&observed));

    let dismissed = EventKind::SurfaceDismissed {
        surface: SnapshotSurface::Alert,
    };
    assert!(!requested.same_variant(&dismissed));
}

#[test]
fn event_kind_parse_rejects_unknown_token() {
    assert!(EventKind::parse("window_opened").is_none());
    assert!(EventKind::parse("bogus").is_none());
}
