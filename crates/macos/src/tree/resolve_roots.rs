use agent_desktop_core::{adapter::SnapshotSurface, error::AdapterError, refs::RefEntry};
use std::time::Instant;

use super::AXElement;
use super::attributes::{
    copy_ax_array, copy_element_attr, copy_i64_attr, copy_string_attr, set_messaging_timeout,
};
use super::element::element_for_pid;
use super::element_dedupe::ElementDedupe;
use super::resolve_deadline::{ensure_before_deadline, remaining_before_deadline};

#[cfg(target_os = "macos")]
pub(super) fn path_candidate_roots(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<Vec<AXElement>, AdapterError> {
    if entry.bounds_hash.is_some() {
        return candidate_roots(entry, deadline);
    }
    Ok(scoped_surface_root(entry, deadline)?.into_iter().collect())
}

#[cfg(target_os = "macos")]
pub(super) fn candidate_roots(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<Vec<AXElement>, AdapterError> {
    if source_window_scope_required(entry) {
        return Ok(exact_source_window_number_root(entry, deadline)?
            .into_iter()
            .collect());
    }

    let root = element_for_pid(entry.pid);
    prepare_for_read(&root, deadline)?;
    let mut roots = Vec::new();
    let mut dedupe = ElementDedupe;
    if let Some(window) = exact_source_window_root(entry, deadline)? {
        dedupe.push(&mut roots, window);
    }
    prepare_for_read(&root, deadline)?;
    if let Some(focused) = copy_element_attr(&root, "AXFocusedWindow") {
        dedupe.push(&mut roots, focused);
    }
    prepare_for_read(&root, deadline)?;
    if let Some(main) = copy_element_attr(&root, "AXMainWindow") {
        dedupe.push(&mut roots, main);
    }
    prepare_for_read(&root, deadline)?;
    for window in copy_ax_array(&root, "AXWindows").unwrap_or_default() {
        dedupe.push(&mut roots, window);
    }
    ensure_before_deadline(deadline)?;
    if let Some(menubar) = crate::tree::menubar_for_pid(entry.pid) {
        dedupe.push(&mut roots, menubar);
    }
    ensure_before_deadline(deadline)?;
    if let Some(menu) = crate::tree::menu_element_for_pid(entry.pid) {
        dedupe.push(&mut roots, menu);
    }
    if roots.is_empty() {
        roots.push(root);
    }
    Ok(roots)
}

#[cfg(target_os = "macos")]
fn scoped_surface_root(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    ensure_before_deadline(deadline)?;
    let root = match entry.source_surface {
        SnapshotSurface::Window if source_window_scope_required(entry) => {
            exact_source_window_number_root(entry, deadline)?
        }
        SnapshotSurface::Window => exact_source_window_root(entry, deadline)?,
        SnapshotSurface::Focused => crate::tree::focused_surface_for_pid(entry.pid),
        SnapshotSurface::Menu => crate::tree::menu_element_for_pid(entry.pid),
        SnapshotSurface::Menubar => crate::tree::menubar_for_pid(entry.pid),
        SnapshotSurface::Sheet => crate::tree::sheet_for_pid(entry.pid),
        SnapshotSurface::Popover => crate::tree::popover_for_pid(entry.pid),
        SnapshotSurface::Alert => crate::tree::alert_for_pid(entry.pid),
    };
    Ok(root)
}

#[cfg(target_os = "macos")]
fn exact_source_window_number_root(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(source_window_number) = source_window_number(entry) else {
        return Ok(None);
    };
    let root = element_for_pid(entry.pid);
    prepare_for_read(&root, deadline)?;
    let Some(windows) = copy_ax_array(&root, "AXWindows") else {
        return Ok(None);
    };
    Ok(windows.into_iter().find(|win| {
        prepare_for_read(win, deadline).is_ok()
            && copy_i64_attr(win, "AXWindowNumber") == Some(source_window_number)
    }))
}

#[cfg(target_os = "macos")]
fn exact_source_window_root(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let root = element_for_pid(entry.pid);
    prepare_for_read(&root, deadline)?;
    let Some(windows) = copy_ax_array(&root, "AXWindows") else {
        return Ok(None);
    };
    if let Some(source_window_number) = source_window_number(entry) {
        if let Some(window) = windows.iter().find(|win| {
            prepare_for_read(win, deadline).is_ok()
                && copy_i64_attr(win, "AXWindowNumber") == Some(source_window_number)
        }) {
            return Ok(Some(window.clone()));
        }
    }
    let Some(source_window_title) = entry.source_window_title.as_deref() else {
        return Ok(None);
    };
    Ok(windows.into_iter().find(|win| {
        prepare_for_read(win, deadline).is_ok()
            && copy_string_attr(win, "AXTitle").as_deref() == Some(source_window_title)
    }))
}

#[cfg(target_os = "macos")]
fn source_window_scope_required(entry: &RefEntry) -> bool {
    matches!(entry.source_surface, SnapshotSurface::Window) && source_window_number(entry).is_some()
}

#[cfg(target_os = "macos")]
pub(super) fn source_window_number(entry: &RefEntry) -> Option<i64> {
    entry
        .source_window_id
        .as_deref()?
        .strip_prefix("w-")?
        .parse()
        .ok()
}

#[cfg(target_os = "macos")]
fn prepare_for_read(element: &AXElement, deadline: Instant) -> Result<(), AdapterError> {
    set_messaging_timeout(element, remaining_before_deadline(deadline)?);
    Ok(())
}
