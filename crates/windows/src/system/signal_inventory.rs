#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use agent_desktop_core::{
    AdapterError, AppInfo, Deadline, ErrorCode, ProcessId, WindowInfo, WindowState,
};

use super::app_ops;
use super::permissions::ensure_budget;
use super::process_identity;
use super::window_enum::{EnumeratedWindow, WindowHandle, enumerate_top_level};
use super::window_identity;
use super::window_ops;

/// How many times a whole pass is re-walked when it catches its own identity
/// inconsistent mid-assembly. Named and sized to match `launch.rs`'s
/// `observe_window_once`, which absorbs the same race over the same inventory.
const LISTING_RACE_ATTEMPTS: u32 = 5;

/// One coherent instant's windows and apps, assembled from a single
/// `EnumWindows` pass plus one `ToolHelp` process snapshot.
///
/// `windows_complete` / `apps_complete` report whether the walk ran to
/// completion, never whether every entity was identifiable.
/// `excluded_window_count` is the separate, observability-only fact: how many
/// admitted windows were dropped for unreadable identity. It never flips
/// either `_complete` bit.
#[derive(Debug, Clone)]
pub(crate) struct SignalWindowInventory {
    pub(crate) windows: Vec<WindowInfo>,
    pub(crate) apps: Vec<AppInfo>,
    pub(crate) windows_complete: bool,
    pub(crate) apps_complete: bool,
    pub(crate) excluded_window_count: usize,
}

impl SignalWindowInventory {
    fn truncated() -> Self {
        Self {
            windows: Vec::new(),
            apps: Vec::new(),
            windows_complete: false,
            apps_complete: false,
            excluded_window_count: 0,
        }
    }
}

struct WalkEntry {
    window: EnumeratedWindow,
    pid: ProcessId,
    token: Option<String>,
}

enum PassOutcome {
    Coherent(SignalWindowInventory),
    Truncated,
}

enum PassError {
    Race,
    Fatal(AdapterError),
}

/// The single-pass, mutually-coherent windows+apps observation `wait --event`
/// polls. A mid-walk identity race is absorbed internally, because surfacing it
/// would abort the whole wait: core retries only `TIMEOUT`, `ELEMENT_NOT_FOUND`
/// and `APP_UNRESPONSIVE`. A truncated walk is reported rather than retried,
/// since retrying it would spend the same exhausted budget again.
pub(crate) fn capture_windows_and_apps(
    deadline: Deadline,
) -> Result<SignalWindowInventory, AdapterError> {
    ensure_budget(deadline)?;
    for _attempt in 0..LISTING_RACE_ATTEMPTS {
        ensure_budget(deadline)?;
        match single_pass(deadline) {
            Ok(PassOutcome::Coherent(inventory)) => return Ok(inventory),
            Ok(PassOutcome::Truncated) => return Ok(SignalWindowInventory::truncated()),
            Err(PassError::Race) => continue,
            Err(PassError::Fatal(error)) => return Err(error),
        }
    }
    Err(mid_walk_identity_race_error(LISTING_RACE_ATTEMPTS))
}

fn single_pass(deadline: Deadline) -> Result<PassOutcome, PassError> {
    #[cfg(test)]
    enum_windows_calls::record();

    let mut candidates = Vec::new();
    let mut truncated = false;
    enumerate_top_level(|window| {
        if !walk_budget_ok(deadline) {
            truncated = true;
            return false;
        }
        if window_ops::passes_filter(&window) {
            candidates.push(window);
        }
        true
    })
    .map_err(|error| PassError::Fatal(remap_native_failure(error)))?;

    if truncated {
        return Ok(PassOutcome::Truncated);
    }

    #[cfg(all(test, target_os = "windows"))]
    if force_race::consume_if_armed() {
        return Err(PassError::Race);
    }

    let walk_entries = walk_phase(candidates, deadline)?;
    assembly_phase(walk_entries, deadline)
}

fn walk_budget_ok(deadline: Deadline) -> bool {
    #[cfg(all(test, target_os = "windows"))]
    if force_truncation::is_active() {
        return false;
    }
    ensure_budget(deadline).is_ok()
}

/// Reads each admitted window's owning pid and process-generation token once
/// per distinct pid, so a process owning many windows costs one token read.
/// A pid that cannot be determined this early means the window has already
/// vanished, not that its identity is permission-blocked, so it is dropped
/// uncounted rather than counted as an identity exclusion; the counted ones are
/// the token and image-name reads, both applied in `assembly_phase`.
fn walk_phase(
    candidates: Vec<EnumeratedWindow>,
    deadline: Deadline,
) -> Result<Vec<WalkEntry>, PassError> {
    let mut cache: HashMap<ProcessId, Option<String>> = HashMap::new();
    let mut entries = Vec::new();
    for (index, window) in candidates.into_iter().enumerate() {
        if index > 0 {
            ensure_budget(deadline).map_err(PassError::Fatal)?;
        }
        let Some(pid) = pid_for_handle(window.handle) else {
            continue;
        };
        let token = cached_token(&mut cache, pid).map_err(PassError::Fatal)?;
        entries.push(WalkEntry { window, pid, token });
    }
    Ok(entries)
}

/// Re-verifies every walk-phase entry against a fresh read and folds in the
/// one `ToolHelp` snapshot for image names.
///
/// Three outcomes, deliberately not the same disposition: a handle that no
/// longer resolves to any pid has simply closed since the walk and is
/// dropped uncounted - its absence is the truth, not an approximation of it.
/// A pid resolving to a *different* identity - a different pid, or the same
/// pid with a different generation token - is the race this pass re-walks for.
/// A token unreadable on both reads is a deterministic exclusion: counted, and
/// never emitted as `None`, which the diff drops and a filtered wait aborts on.
fn assembly_phase(entries: Vec<WalkEntry>, deadline: Deadline) -> Result<PassOutcome, PassError> {
    ensure_budget(deadline).map_err(PassError::Fatal)?;
    let snapshot = app_ops::process_snapshot()
        .map_err(|error| PassError::Fatal(remap_native_failure(error)))?;
    let name_by_pid: HashMap<ProcessId, String> = snapshot
        .into_iter()
        .map(|row| (row.pid, row.name))
        .collect();

    let mut cache: HashMap<ProcessId, Option<String>> = HashMap::new();
    let mut windows = Vec::new();
    let mut apps = Vec::new();
    let mut apps_seen: HashSet<ProcessId> = HashSet::new();
    let mut excluded_window_count = 0usize;
    let mut focused_seen = false;

    for (index, entry) in entries.into_iter().enumerate() {
        if index > 0 {
            ensure_budget(deadline).map_err(PassError::Fatal)?;
        }
        let Some(current_pid) = pid_for_handle(entry.window.handle) else {
            continue;
        };
        if current_pid != entry.pid {
            return Err(PassError::Race);
        }
        let fresh_token = cached_token(&mut cache, current_pid).map_err(PassError::Fatal)?;
        let token = match (entry.token.as_deref(), fresh_token.as_deref()) {
            (None, None) => {
                excluded_window_count += 1;
                continue;
            }
            (Some(walk_token), Some(assembly_token)) if walk_token == assembly_token => {
                walk_token.to_string()
            }
            _ => return Err(PassError::Race),
        };
        let Some(app_name) = name_by_pid.get(&current_pid).cloned() else {
            excluded_window_count += 1;
            continue;
        };

        let focused = !focused_seen && window_ops::is_foreground_window(entry.window.handle);
        focused_seen |= focused;
        windows.push(WindowInfo {
            id: format!("w-{}", entry.window.handle as usize),
            title: window_identity::live_window_title(entry.window.handle).unwrap_or_default(),
            app: app_name.clone(),
            pid: current_pid,
            process_instance: Some(token.clone()),
            bounds: Some(entry.window.rect),
            state: WindowState {
                is_focused: focused,
                minimized: Some(entry.window.iconic),
                visible: Some(entry.window.visible),
            },
        });
        if apps_seen.insert(current_pid) {
            apps.push(AppInfo {
                name: app_name,
                pid: current_pid,
                bundle_id: None,
                process_instance: Some(token),
                presentation: None,
            });
        }
    }
    apps.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(PassOutcome::Coherent(SignalWindowInventory {
        windows,
        apps,
        windows_complete: true,
        apps_complete: true,
        excluded_window_count,
    }))
}

fn cached_token(
    cache: &mut HashMap<ProcessId, Option<String>>,
    pid: ProcessId,
) -> Result<Option<String>, AdapterError> {
    if let Some(cached) = cache.get(&pid) {
        return Ok(cached.clone());
    }
    let token = token_for_pid_seamed(pid)?;
    cache.insert(pid, token.clone());
    Ok(token)
}

fn token_for_pid_seamed(pid: ProcessId) -> Result<Option<String>, AdapterError> {
    #[cfg(test)]
    if force_token_none::matches(pid) {
        return Ok(None);
    }
    process_identity::token_for_pid(pid)
}

#[cfg(target_os = "windows")]
fn pid_for_handle(handle: WindowHandle) -> Option<ProcessId> {
    use windows_sys::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    let mut pid: u32 = 0;
    unsafe { GetWindowThreadProcessId(handle, &mut pid) };
    if pid == 0 {
        None
    } else {
        Some(ProcessId::from(pid))
    }
}

#[cfg(not(target_os = "windows"))]
fn pid_for_handle(_handle: WindowHandle) -> Option<ProcessId> {
    None
}

/// Narrows any native-call failure onto the closed error set. `TIMEOUT`
/// passes through unchanged; everything else - `EnumWindows` or `ToolHelp`
/// reporting `INTERNAL` - becomes `APP_UNRESPONSIVE`, since the caller
/// treats both as "the desktop could not be read right now, try again."
fn remap_native_failure(error: AdapterError) -> AdapterError {
    if error.code == ErrorCode::Timeout {
        return error;
    }
    let mut mapped = AdapterError::new(ErrorCode::AppUnresponsive, error.message.clone())
        .with_details(serde_json::json!({ "kind": "native_enumeration_failure" }));
    if let Some(detail) = error.platform_detail.as_deref() {
        mapped = mapped.with_platform_detail(detail.to_string());
    }
    mapped
}

fn mid_walk_identity_race_error(attempts: u32) -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "Window identity was inconsistent across every re-walk attempt",
    )
    .with_details(serde_json::json!({
        "kind": "mid_walk_identity_race",
        "attempts": attempts,
    }))
}

#[cfg(test)]
pub(super) mod enum_windows_calls {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn record() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(super) fn take() -> usize {
        COUNT.with(|cell| {
            let value = cell.get();
            cell.set(0);
            value
        })
    }
}

#[cfg(test)]
pub(super) mod force_token_none {
    use std::cell::Cell;

    use agent_desktop_core::ProcessId;

    thread_local! {
        static TARGET: Cell<Option<u32>> = const { Cell::new(None) };
    }

    struct ResetOnDrop;

    impl Drop for ResetOnDrop {
        fn drop(&mut self) {
            TARGET.with(|cell| cell.set(None));
        }
    }

    pub(super) fn with<R>(pid: ProcessId, run: impl FnOnce() -> R) -> R {
        TARGET.with(|cell| cell.set(Some(u32::from(pid))));
        let _reset = ResetOnDrop;
        run()
    }

    pub(super) fn matches(pid: ProcessId) -> bool {
        TARGET.with(|cell| cell.get() == Some(u32::from(pid)))
    }
}

#[cfg(all(test, target_os = "windows"))]
pub(super) mod force_race {
    use std::cell::Cell;

    thread_local! {
        static REMAINING: Cell<usize> = const { Cell::new(0) };
    }

    pub(super) fn with<R>(times: usize, run: impl FnOnce() -> R) -> R {
        crate::system::test_support::with_usize_flag(&REMAINING, times, run)
    }

    pub(super) fn consume_if_armed() -> bool {
        REMAINING.with(|cell| {
            let remaining = cell.get();
            if remaining == 0 {
                false
            } else {
                cell.set(remaining - 1);
                true
            }
        })
    }
}

#[cfg(all(test, target_os = "windows"))]
pub(super) mod force_truncation {
    use std::cell::Cell;

    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
    }

    pub(super) fn with<R>(run: impl FnOnce() -> R) -> R {
        crate::system::test_support::with_flag(&ACTIVE, true, run)
    }

    pub(super) fn is_active() -> bool {
        ACTIVE.with(Cell::get)
    }
}

#[cfg(test)]
#[path = "signal_inventory_tests.rs"]
mod tests;
