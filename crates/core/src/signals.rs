#[cfg(test)]
use crate::snapshot_surface::SnapshotSurface;
use crate::{AppInfo, EventKind, ProcessId, SignalBaseline, SurfaceSignal, UiEvent, WindowInfo};
use std::collections::HashSet;

/// Pure baseline-diff over two independently captured [`SignalBaseline`]
/// snapshots. Never touches the adapter — every code path here is exercised
/// with hand-built fixtures, which is what makes it a real regression guard
/// for F8/F9-shaped bugs (wrong-id matches, masked closes) instead of one
/// that can only be verified by hand against a live desktop.
pub fn diff_signals(baseline: &SignalBaseline, current: &SignalBaseline) -> Vec<UiEvent> {
    let mut events = Vec::new();
    if baseline.completeness.windows && current.completeness.windows {
        diff_windows(baseline, current, &mut events);
        diff_focus(baseline, current, &mut events);
    }
    if baseline.completeness.apps && current.completeness.apps {
        diff_apps(baseline, current, &mut events);
    }
    if baseline.completeness.surfaces && current.completeness.surfaces {
        diff_surfaces(baseline, current, &mut events);
    }
    events.sort_by(compare_events);
    events
}

fn compare_events(left: &UiEvent, right: &UiEvent) -> std::cmp::Ordering {
    event_rank(&left.kind)
        .cmp(&event_rank(&right.kind))
        .then_with(|| left.app.cmp(&right.app))
        .then_with(|| left.pid.cmp(&right.pid))
        .then_with(|| left.window_id.cmp(&right.window_id))
        .then_with(|| left.title.cmp(&right.title))
        .then_with(|| left.kind.cli_token().cmp(right.kind.cli_token()))
}

fn event_rank(kind: &EventKind) -> u8 {
    match kind {
        EventKind::WindowClosed => 0,
        EventKind::AppTerminated => 1,
        EventKind::SurfaceDismissed { .. } => 2,
        EventKind::WindowOpened => 3,
        EventKind::AppLaunched => 4,
        EventKind::SurfaceAppeared { .. } => 5,
        EventKind::FocusChangedWindow => 6,
    }
}

type WindowIdentity<'a> = (ProcessId, &'a str, &'a str);

fn window_identity(window: &WindowInfo) -> Option<WindowIdentity<'_>> {
    window
        .process_instance
        .as_deref()
        .map(|instance| (window.pid, instance, window.id.as_str()))
}

fn window_event(kind: EventKind, win: &WindowInfo) -> UiEvent {
    UiEvent {
        kind,
        window_id: Some(win.id.clone()),
        title: Some(win.title.clone()),
        app: Some(win.app.clone()),
        pid: Some(win.pid),
    }
}

fn diff_windows(baseline: &SignalBaseline, current: &SignalBaseline, events: &mut Vec<UiEvent>) {
    let baseline_ids: HashSet<WindowIdentity<'_>> = baseline
        .windows
        .iter()
        .filter_map(window_identity)
        .collect();
    for win in &current.windows {
        if window_identity(win).is_some_and(|identity| !baseline_ids.contains(&identity)) {
            events.push(window_event(EventKind::WindowOpened, win));
        }
    }
    let current_ids: HashSet<WindowIdentity<'_>> =
        current.windows.iter().filter_map(window_identity).collect();
    for win in &baseline.windows {
        if window_identity(win).is_some_and(|identity| !current_ids.contains(&identity)) {
            events.push(window_event(EventKind::WindowClosed, win));
        }
    }
}

fn diff_focus(baseline: &SignalBaseline, current: &SignalBaseline, events: &mut Vec<UiEvent>) {
    let baseline_focused_id = baseline
        .windows
        .iter()
        .find(|w| w.state.is_focused)
        .and_then(window_identity);
    let current_focused = current
        .windows
        .iter()
        .find(|window| window.state.is_focused);
    match current_focused {
        Some(window)
            if window_identity(window)
                .is_some_and(|identity| Some(identity) != baseline_focused_id) =>
        {
            events.push(window_event(EventKind::FocusChangedWindow, window));
        }
        None if baseline_focused_id.is_some() => events.push(UiEvent {
            kind: EventKind::FocusChangedWindow,
            window_id: None,
            title: None,
            app: None,
            pid: None,
        }),
        Some(_) | None => {}
    }
}

fn app_event(kind: EventKind, app: &AppInfo) -> UiEvent {
    UiEvent {
        kind,
        window_id: None,
        title: None,
        app: Some(app.name.clone()),
        pid: Some(app.pid),
    }
}

fn diff_apps(baseline: &SignalBaseline, current: &SignalBaseline, events: &mut Vec<UiEvent>) {
    let baseline_pids: HashSet<_> = baseline.apps.iter().filter_map(app_identity).collect();
    for app in &current.apps {
        if app_identity(app).is_some_and(|identity| !baseline_pids.contains(&identity)) {
            events.push(app_event(EventKind::AppLaunched, app));
        }
    }
    let current_pids: HashSet<_> = current.apps.iter().filter_map(app_identity).collect();
    for app in &baseline.apps {
        if app_identity(app).is_some_and(|identity| !current_pids.contains(&identity)) {
            events.push(app_event(EventKind::AppTerminated, app));
        }
    }
}

fn app_identity(app: &AppInfo) -> Option<(ProcessId, &str)> {
    app.process_instance
        .as_deref()
        .map(|instance| (app.pid, instance))
}

type SurfaceKey<'a> = (ProcessId, &'a str, &'a str, &'static str);

fn surface_identity(surface: &SurfaceSignal) -> SurfaceKey<'_> {
    (
        surface.pid,
        surface.process_instance.as_str(),
        surface.id.as_str(),
        surface.kind.as_str(),
    )
}

fn surface_event(surface: &SurfaceSignal, appeared: bool) -> UiEvent {
    UiEvent {
        kind: if appeared {
            EventKind::SurfaceAppeared {
                surface: surface.kind,
            }
        } else {
            EventKind::SurfaceDismissed {
                surface: surface.kind,
            }
        },
        window_id: None,
        title: surface.title.clone(),
        app: Some(surface.app.clone()),
        pid: Some(surface.pid),
    }
}

fn diff_surfaces(baseline: &SignalBaseline, current: &SignalBaseline, events: &mut Vec<UiEvent>) {
    let baseline_ids: HashSet<_> = baseline.surfaces.iter().map(surface_identity).collect();
    for surface in &current.surfaces {
        if !baseline_ids.contains(&surface_identity(surface)) {
            events.push(surface_event(surface, true));
        }
    }
    let current_ids: HashSet<_> = current.surfaces.iter().map(surface_identity).collect();
    for surface in &baseline.surfaces {
        if !current_ids.contains(&surface_identity(surface)) {
            events.push(surface_event(surface, false));
        }
    }
}

/// Grows `seen` with any entry from `current` not already present, keyed by
/// the same identity `diff_signals` uses. Nothing already in `seen` is ever
/// removed: this is the running union a disappearance-class wait diffs
/// against, so an entity that both appeared and disappeared within one wait
/// is still detected even though it never appears in the wait's original
/// fixed baseline.
pub(crate) fn merge_signal_baseline(
    seen: &SignalBaseline,
    current: &SignalBaseline,
) -> SignalBaseline {
    let seen_window_ids: HashSet<WindowIdentity<'_>> =
        seen.windows.iter().filter_map(window_identity).collect();
    let mut windows = seen.windows.clone();
    for win in &current.windows {
        if window_identity(win).is_some_and(|identity| !seen_window_ids.contains(&identity)) {
            windows.push(win.clone());
        }
    }

    let seen_app_ids: HashSet<_> = seen.apps.iter().filter_map(app_identity).collect();
    let mut apps = seen.apps.clone();
    for app in &current.apps {
        if app_identity(app).is_some_and(|identity| !seen_app_ids.contains(&identity)) {
            apps.push(app.clone());
        }
    }

    let seen_surface_ids: HashSet<_> = seen.surfaces.iter().map(surface_identity).collect();
    let mut surfaces = seen.surfaces.clone();
    for surface in &current.surfaces {
        if !seen_surface_ids.contains(&surface_identity(surface)) {
            surfaces.push(surface.clone());
        }
    }

    SignalBaseline {
        windows,
        apps,
        surfaces,
        completeness: current.completeness,
    }
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod tests;
