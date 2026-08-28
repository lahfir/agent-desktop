use agent_desktop_core::{
    AdapterError, Deadline, ErrorCode, ProcessId, ProcessIdentity, SnapshotSurface, SurfaceInfo,
};

use super::automation::root_from_hwnd;
use super::chromium;
use super::surfaces::window_is_modal_sheet;
use crate::system::menu_state::{MenuLocation, locate_menu};
use crate::system::window_enum::{EnumeratedWindow, WindowHandle, enumerate_top_level};
use crate::system::window_identity::{live_window_owner, live_window_title};
use crate::system::window_ops::{is_foreground_window, passes_filter};

/// The surfaces the named process presents right now.
///
/// Every agent-facing top-level window is a `window` surface, the foreground
/// one is also `focused`, a window whose `WindowIsModal` reads true is a
/// `sheet`, and a process with an open menu presents one `menu` surface. The
/// window-backed surfaces carry the same `w-<hwnd>` identity `list_windows`
/// emits, so a surface id is consumed by the window observation path without
/// a second lookup.
///
/// The classifications are the surface path's own - `window_is_modal_sheet`
/// and the `menu_state` detector - so this inventory and `snapshot --surface`
/// can never disagree about what a window or a menu is. Shell surfaces are
/// not in a per-process inventory: they belong to the shell, not to any named
/// process, and folding them in would make every process appear to own the
/// taskbar. A process with no windows answers an empty list, which is a
/// successful answer about that process, not a refusal.
pub(crate) fn list_surfaces_for_process(
    process: ProcessIdentity,
    deadline: Deadline,
) -> Result<Vec<SurfaceInfo>, AdapterError> {
    crate::system::permissions::ensure_budget(deadline)?;
    require_live_process(&process)?;
    let windows = windows_of(process.pid, deadline)?;
    let mut surfaces = Vec::new();
    for window in &windows {
        let title = live_window_title(window.handle);
        surfaces.push(surface(
            SnapshotSurface::Window,
            window.handle,
            title.clone(),
        ));
        if is_foreground_window(window.handle) {
            surfaces.push(surface(
                SnapshotSurface::Focused,
                window.handle,
                title.clone(),
            ));
        }
        if is_modal_sheet(window.handle, deadline)? {
            surfaces.push(surface(SnapshotSurface::Sheet, window.handle, title));
        }
    }
    crate::system::permissions::ensure_budget(deadline)?;
    if let Some(menu) = locate_menu(process.pid, deadline)? {
        surfaces.push(menu_surface(&menu)?);
    }
    Ok(surfaces)
}

/// The caller's generation token must still be the process at `pid` right
/// now, the same fail-closed corroboration every other named-process entry
/// point on this adapter applies before answering.
fn require_live_process(process: &ProcessIdentity) -> Result<(), AdapterError> {
    if crate::system::process_identity::matches_instance(process.pid, &process.instance)? {
        return Ok(());
    }
    Err(AdapterError::new(
        ErrorCode::AppNotFound,
        "The named process is not running, or the pid now runs a different generation",
    ))
}

/// The pid's agent-facing top-level windows, from the one enumeration pass
/// every window inventory on this platform shares - the same walk
/// `list_windows` and the menu detector draw from, filtered by the same
/// census predicate.
fn windows_of(pid: ProcessId, deadline: Deadline) -> Result<Vec<EnumeratedWindow>, AdapterError> {
    let mut windows = Vec::new();
    let failure = std::cell::RefCell::new(None);
    let mut seen = 0usize;
    enumerate_top_level(|window| {
        if seen > 0 {
            if let Err(error) = crate::system::permissions::ensure_budget(deadline) {
                *failure.borrow_mut() = Some(error);
                return false;
            }
        }
        seen += 1;
        if live_window_owner(window.handle) == Some(pid) && passes_filter(&window) {
            windows.push(window);
        }
        true
    })?;
    if let Some(error) = failure.into_inner() {
        return Err(error);
    }
    Ok(windows)
}

fn surface(kind: SnapshotSurface, handle: WindowHandle, title: Option<String>) -> SurfaceInfo {
    SurfaceInfo {
        id: format!("w-{}", handle as usize),
        kind: kind.as_str().to_string(),
        title,
        item_count: None,
    }
}

/// The modal classification is the surface path's own predicate, read from
/// the window's UIA root exactly as `snapshot --surface sheet` reads it.
fn is_modal_sheet(handle: WindowHandle, deadline: Deadline) -> Result<bool, AdapterError> {
    let root = root_from_hwnd(handle as isize, deadline)?;
    Ok(window_is_modal_sheet(
        &root,
        chromium::is_chromium_root(&root),
    ))
}

fn menu_surface(menu: &MenuLocation) -> Result<SurfaceInfo, AdapterError> {
    let (title, item_count) = menu_decorations(menu);
    Ok(SurfaceInfo {
        id: format!("w-{}", menu.root_handle()),
        kind: SnapshotSurface::Menu.as_str().to_string(),
        title,
        item_count,
    })
}

/// Both decorations are reads on the element the detector already located and
/// proved reachable, so a read that races the menu closing omits its field
/// rather than failing the listing - `SurfaceInfo` skips `None`.
#[cfg(target_os = "windows")]
fn menu_decorations(menu: &MenuLocation) -> (Option<String>, Option<usize>) {
    let title = menu.element.get_name().ok().filter(|name| !name.is_empty());
    (title, menu_item_count(menu).ok())
}

#[cfg(not(target_os = "windows"))]
fn menu_decorations(_menu: &MenuLocation) -> (Option<String>, Option<usize>) {
    (None, None)
}

/// One scoped child read of the already-located element: the detector's walk
/// found this element, and counting its direct items is the follow-up that
/// populates `item_count` where the surface has countable children. No second
/// search of the desktop happens - the element is in hand.
#[cfg(target_os = "windows")]
fn menu_item_count(menu: &MenuLocation) -> Result<usize, AdapterError> {
    use uiautomation::types::TreeScope;

    let client = crate::tree::automation::automation_client()?;
    let condition = client.create_true_condition().map_err(|error| {
        crate::tree::automation::uia_error(&error, "build the menu item-count condition")
    })?;
    let items = menu
        .element
        .find_all(TreeScope::Children, &condition)
        .map_err(|error| {
            crate::tree::automation::uia_error(&error, "read the located menu's items")
        })?;
    Ok(items.len())
}

#[cfg(test)]
#[path = "surface_inventory_tests.rs"]
mod tests;
