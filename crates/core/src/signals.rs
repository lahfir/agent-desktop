use crate::{
    node::{AppInfo, WindowInfo},
    snapshot_surface::SnapshotSurface,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default)]
pub struct SignalFilter {
    pub app: Option<String>,
    pub pid: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SurfaceSignal {
    pub kind: SnapshotSurface,
    pub app: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SignalBaseline {
    pub windows: Vec<WindowInfo>,
    pub apps: Vec<AppInfo>,
    pub surfaces: Vec<SurfaceSignal>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventKind {
    WindowOpened,
    WindowClosed,
    AppLaunched,
    AppTerminated,
    FocusChangedWindow,
    SurfaceAppeared { surface: SnapshotSurface },
    SurfaceDismissed { surface: SnapshotSurface },
}

impl EventKind {
    /// Ignores payload fields (e.g. the `surface` carried by `SurfaceAppeared`)
    /// so a caller-supplied request template — which does not know which
    /// surface kind will show up — still matches the concrete observed event.
    pub fn same_variant(&self, other: &EventKind) -> bool {
        std::mem::discriminant(self) == std::mem::discriminant(other)
    }

    pub fn cli_token(&self) -> &'static str {
        match self {
            EventKind::WindowOpened => "window-opened",
            EventKind::WindowClosed => "window-closed",
            EventKind::AppLaunched => "app-launched",
            EventKind::AppTerminated => "app-terminated",
            EventKind::FocusChangedWindow => "focus-changed",
            EventKind::SurfaceAppeared { .. } => "surface-appeared",
            EventKind::SurfaceDismissed { .. } => "surface-dismissed",
        }
    }

    /// Every known `--event` token, in the order CLI help/error messages list them.
    pub fn all_tokens() -> &'static [&'static str] {
        &[
            "window-opened",
            "window-closed",
            "app-launched",
            "app-terminated",
            "focus-changed",
            "surface-appeared",
            "surface-dismissed",
        ]
    }

    /// Parses a `--event` token into a match template. `SurfaceAppeared`/
    /// `SurfaceDismissed` templates carry a placeholder surface kind — it is
    /// never inspected by `same_variant`, only the discriminant is.
    pub fn parse(token: &str) -> Option<EventKind> {
        match token {
            "window-opened" => Some(EventKind::WindowOpened),
            "window-closed" => Some(EventKind::WindowClosed),
            "app-launched" => Some(EventKind::AppLaunched),
            "app-terminated" => Some(EventKind::AppTerminated),
            "focus-changed" => Some(EventKind::FocusChangedWindow),
            "surface-appeared" => Some(EventKind::SurfaceAppeared {
                surface: SnapshotSurface::Window,
            }),
            "surface-dismissed" => Some(EventKind::SurfaceDismissed {
                surface: SnapshotSurface::Window,
            }),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UiEvent {
    #[serde(flatten)]
    pub kind: EventKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<i32>,
}

/// Pure baseline-diff over two independently captured [`SignalBaseline`]
/// snapshots. Never touches the adapter — every code path here is exercised
/// with hand-built fixtures, which is what makes it a real regression guard
/// for F8/F9-shaped bugs (wrong-id matches, masked closes) instead of one
/// that can only be verified by hand against a live desktop.
pub fn diff_signals(baseline: &SignalBaseline, current: &SignalBaseline) -> Vec<UiEvent> {
    let mut events = Vec::new();
    diff_windows(baseline, current, &mut events);
    diff_focus(baseline, current, &mut events);
    diff_apps(baseline, current, &mut events);
    diff_surfaces(baseline, current, &mut events);
    events
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
    let baseline_ids: HashSet<&str> = baseline.windows.iter().map(|w| w.id.as_str()).collect();
    for win in &current.windows {
        if !baseline_ids.contains(win.id.as_str()) {
            events.push(window_event(EventKind::WindowOpened, win));
        }
    }
    let current_ids: HashSet<&str> = current.windows.iter().map(|w| w.id.as_str()).collect();
    for win in &baseline.windows {
        if !current_ids.contains(win.id.as_str()) {
            events.push(window_event(EventKind::WindowClosed, win));
        }
    }
}

fn diff_focus(baseline: &SignalBaseline, current: &SignalBaseline, events: &mut Vec<UiEvent>) {
    let baseline_focused_id = baseline
        .windows
        .iter()
        .find(|w| w.is_focused)
        .map(|w| w.id.as_str());
    if let Some(win) = current.windows.iter().find(|w| w.is_focused) {
        if Some(win.id.as_str()) != baseline_focused_id {
            events.push(window_event(EventKind::FocusChangedWindow, win));
        }
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
    let baseline_pids: HashSet<i32> = baseline.apps.iter().map(|a| a.pid).collect();
    for app in &current.apps {
        if !baseline_pids.contains(&app.pid) {
            events.push(app_event(EventKind::AppLaunched, app));
        }
    }
    let current_pids: HashSet<i32> = current.apps.iter().map(|a| a.pid).collect();
    for app in &baseline.apps {
        if !current_pids.contains(&app.pid) {
            events.push(app_event(EventKind::AppTerminated, app));
        }
    }
}

type SurfaceCounts = HashMap<(&'static str, String), usize>;

fn surface_counts(surfaces: &[SurfaceSignal]) -> SurfaceCounts {
    let mut counts: SurfaceCounts = HashMap::new();
    for surface in surfaces {
        *counts
            .entry((surface.kind.as_str(), surface.app.clone()))
            .or_insert(0) += 1;
    }
    counts
}

fn surface_event(kind: SnapshotSurface, app: &str, appeared: bool) -> UiEvent {
    UiEvent {
        kind: if appeared {
            EventKind::SurfaceAppeared { surface: kind }
        } else {
            EventKind::SurfaceDismissed { surface: kind }
        },
        window_id: None,
        title: None,
        app: Some(app.to_string()),
        pid: None,
    }
}

/// Counting-map diff (survives reordering, matches the `NotificationFingerprint`
/// pattern) since surfaces have no stable id — only a `(kind, app)` shape.
fn diff_surfaces(baseline: &SignalBaseline, current: &SignalBaseline, events: &mut Vec<UiEvent>) {
    let baseline_counts = surface_counts(&baseline.surfaces);
    let current_counts = surface_counts(&current.surfaces);

    for (key, count) in &current_counts {
        let prior = baseline_counts.get(key).copied().unwrap_or(0);
        if *count > prior {
            let kind = surface_kind_from_str(key.0);
            for _ in 0..(*count - prior) {
                events.push(surface_event(kind, &key.1, true));
            }
        }
    }
    for (key, count) in &baseline_counts {
        let now = current_counts.get(key).copied().unwrap_or(0);
        if *count > now {
            let kind = surface_kind_from_str(key.0);
            for _ in 0..(*count - now) {
                events.push(surface_event(kind, &key.1, false));
            }
        }
    }
}

fn surface_kind_from_str(raw: &str) -> SnapshotSurface {
    match raw {
        "focused" => SnapshotSurface::Focused,
        "menu" => SnapshotSurface::Menu,
        "menubar" => SnapshotSurface::Menubar,
        "sheet" => SnapshotSurface::Sheet,
        "popover" => SnapshotSurface::Popover,
        "alert" => SnapshotSurface::Alert,
        _ => SnapshotSurface::Window,
    }
}

#[cfg(test)]
#[path = "signals_tests.rs"]
mod tests;
