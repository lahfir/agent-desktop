use agent_desktop_core::{AdapterError, WindowFilter, WindowInfo};
use std::time::Instant;

use crate::system::cg_window;

#[cfg(test)]
fn apps_from_window_records(
    records: &[cg_window::WindowRecord],
) -> Vec<agent_desktop_core::AppInfo> {
    let mut seen_pids = rustc_hash::FxHashSet::default();
    let mut apps = Vec::new();

    for record in records {
        if !seen_pids.insert(record.pid) {
            continue;
        }
        apps.push(agent_desktop_core::AppInfo {
            name: record.app_name.clone(),
            pid: agent_desktop_core::ProcessId::try_from(record.pid)
                .expect("fixture pid must be positive"),
            bundle_id: None,
            process_instance: record.process_instance.clone(),
            presentation: None,
        });
    }

    apps
}

pub(crate) fn list_windows_until(
    filter: &WindowFilter,
    deadline: Instant,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let app_filter = filter.app.as_deref().unwrap_or("").to_ascii_lowercase();
    if app_filter.is_empty() {
        return crate::system::window_inventory_global::list_windows_until(
            filter.focused_only,
            deadline,
        );
    }
    let scope = cg_window::WindowRecordScope::App(&app_filter);
    let records = cg_window::window_records_until(deadline, scope)?;
    let candidates = records;
    windows_from_records_with_focus(
        candidates,
        filter.focused_only,
        |pid| crate::system::window_ax_state::read_until(pid, deadline),
        crate::system::process_identity::matches_instance,
    )
}

/// The window server already knows which windows exist; accessibility is only
/// asked which one holds focus and which are minimized. An application too busy
/// to answer that — a freshly launched one, or one sitting in a modal panel —
/// must not hide its windows from the inventory, or a caller waits out its whole
/// deadline for windows the window server listed immediately. Selecting *the
/// focused* window is the one case that genuinely needs the answer.
fn focus_state(
    ax_state: &mut impl FnMut(
        i32,
    )
        -> Result<crate::system::window_ax_state::WindowAxState, AdapterError>,
    pid: i32,
    focused_only: bool,
) -> Result<crate::system::window_ax_state::WindowAxState, AdapterError> {
    match ax_state(pid) {
        Ok(state) => Ok(state),
        Err(error) if focused_only => Err(error),
        Err(_) => Ok(crate::system::window_ax_state::WindowAxState {
            focused: None,
            minimized_by_id: rustc_hash::FxHashMap::default(),
        }),
    }
}

/// An application carries offscreen bookkeeping windows alongside the one the
/// user sees — TextEdit answers with four menu-bar-sized panels plus a save
/// accessory view before its first document appears. Counting those makes the
/// only real window look ambiguous, and reports an application that has not
/// drawn anything yet as having several windows to choose between.
///
/// A minimized window is the user's window and is kept: it is offscreen for a
/// reason the caller cares about, unlike a panel that was never meant to be
/// seen. Dropping it would report an application as having no window while its
/// window sits in the Dock.
fn narrow_to_real_windows(windows: &mut Vec<WindowInfo>) {
    windows.retain(|window| {
        window.state.visible == Some(true) || window.state.minimized == Some(true)
    });
}

pub(crate) fn exact_window_for_pid_until(
    pid: i32,
    deadline: Instant,
) -> Result<WindowInfo, AdapterError> {
    let records =
        cg_window::window_records_until(deadline, cg_window::WindowRecordScope::Pid(pid))?;
    let mut windows = windows_from_records_with_focus(
        records,
        false,
        |owner_pid| crate::system::window_ax_state::read_until(owner_pid, deadline),
        |owner_pid, instance| {
            crate::system::process_identity::matches_instance(owner_pid, instance)
        },
    )?;
    narrow_to_real_windows(&mut windows);
    if windows.len() == 1 {
        return Ok(windows.remove(0));
    }
    let mut focused = windows
        .iter()
        .filter(|window| window.state.is_focused)
        .cloned()
        .collect::<Vec<_>>();
    if focused.len() == 1 {
        return Ok(focused.remove(0));
    }
    if windows.is_empty() {
        return Err(AdapterError::new(
            agent_desktop_core::ErrorCode::WindowNotFound,
            format!("Process {pid} has no visible window"),
        ));
    }
    Err(AdapterError::ambiguous_target(format!(
        "Process {pid} has multiple visible windows and no exact focused window"
    ))
    .with_details(serde_json::json!({
        "candidate_count": windows.len(),
        "candidate_window_ids": windows.iter().map(|window| &window.id).collect::<Vec<_>>(),
    })))
}

fn windows_from_records_with_focus(
    records: Vec<cg_window::WindowRecord>,
    focused_only: bool,
    mut ax_state: impl FnMut(i32) -> Result<crate::system::window_ax_state::WindowAxState, AdapterError>,
    mut verify_instance: impl FnMut(i32, &str) -> Result<bool, AdapterError>,
) -> Result<Vec<WindowInfo>, AdapterError> {
    let candidates = records
        .into_iter()
        .filter(|record| record.window_number > 0)
        .collect::<Vec<_>>();
    let mut title_counts = std::collections::HashMap::new();
    for record in &candidates {
        *title_counts
            .entry((record.pid, record.display_title().to_owned()))
            .or_insert(0) += 1;
    }

    let mut state_cache = std::collections::HashMap::new();
    let mut windows = Vec::new();
    let mut focused_seen = false;

    for record in candidates {
        let process_instance = record.process_instance.as_deref().ok_or_else(|| {
            identity_race_error(
                record.pid,
                "CoreGraphics record omitted its process instance",
            )
        })?;
        if !verify_instance(record.pid, process_instance)? {
            return Err(identity_race_error(
                record.pid,
                "window owner changed before accessibility focus read",
            ));
        }
        let title_count = title_counts
            .get(&(record.pid, record.display_title().to_owned()))
            .copied()
            .unwrap_or(0);
        let state = match state_cache.entry(record.pid) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(focus_state(&mut ax_state, record.pid, focused_only)?)
            }
        };
        if !verify_instance(record.pid, process_instance)? {
            return Err(identity_race_error(
                record.pid,
                "window owner changed after accessibility focus read",
            ));
        }
        let is_focused = !focused_seen
            && matches_focused_window(
                record.display_title(),
                record.window_number,
                &state.focused,
                title_count,
            );
        if focused_only && !is_focused {
            continue;
        }
        focused_seen |= is_focused;
        let minimized = state.minimized_by_id.get(&record.window_number).copied();
        windows.push(record.into_window_info(is_focused, minimized)?);
    }

    Ok(windows)
}

#[cfg(test)]
fn matches_app_filter(app_name: &str, app_filter: &str) -> bool {
    app_filter.is_empty() || app_name.eq_ignore_ascii_case(app_filter)
}

fn identity_race_error(pid: i32, phase: &str) -> AdapterError {
    AdapterError::new(
        agent_desktop_core::ErrorCode::AppUnresponsive,
        "Window owner changed while exact window identity was being assembled",
    )
    .with_details(serde_json::json!({
        "kind": "window_identity_race",
        "pid": pid,
        "phase": phase,
        "complete": false,
        "retryable": true,
    }))
}

type FocusedWindowIdentity = Option<(Option<String>, Option<i64>)>;

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

#[cfg(test)]
#[path = "window_inventory_tests.rs"]
mod tests;
