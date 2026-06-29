use agent_desktop_core::{adapter::SnapshotSurface, error::AdapterError, refs::RefEntry};
use std::time::Instant;

use super::AXElement;
use super::attributes::{
    copy_ax_array, copy_element_attr, copy_i64_attr, copy_string_attr, set_messaging_timeout,
};
use super::element::element_for_pid;
use super::element_dedupe::ElementDedupe;
use super::resolve_deadline::{ensure_before_deadline, remaining_before_deadline};
use super::resolve_identity::bounded_window_fallback_allowed;

#[cfg(target_os = "macos")]
pub(super) struct CandidateRoots {
    pub roots: Vec<AXElement>,
    pub scope_verified: bool,
}

#[cfg(target_os = "macos")]
pub(super) fn path_candidate_roots(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<CandidateRoots, AdapterError> {
    if entry.bounds_hash.is_some() {
        return candidate_roots(entry, deadline);
    }
    let roots: Vec<_> = scoped_surface_root(entry, deadline)?.into_iter().collect();
    Ok(CandidateRoots {
        scope_verified: source_window_scope_required(entry) && !roots.is_empty(),
        roots,
    })
}

#[cfg(target_os = "macos")]
pub(super) fn candidate_roots(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<CandidateRoots, AdapterError> {
    if source_window_scope_required(entry) {
        return source_window_scoped_roots(entry, deadline);
    }

    let root = element_for_pid(entry.pid);
    prepare_for_read(&root, deadline)?;
    let mut roots = Vec::new();
    let mut dedupe = ElementDedupe;
    let windows = copy_ax_array(&root, "AXWindows").unwrap_or_default();
    if let Some(window) = exact_source_window_from_windows(&windows, entry, deadline)? {
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
    for window in windows {
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
    Ok(CandidateRoots {
        roots,
        scope_verified: false,
    })
}

#[cfg(target_os = "macos")]
fn source_window_scoped_roots(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<CandidateRoots, AdapterError> {
    let Some(windows) = windows_for_pid(entry.pid, deadline)? else {
        return Ok(CandidateRoots {
            roots: Vec::new(),
            scope_verified: false,
        });
    };
    if let Some(window) = window_by_number(&windows, source_window_number(entry), deadline)? {
        return Ok(CandidateRoots {
            roots: vec![window],
            scope_verified: true,
        });
    }
    if let Some(window) = window_by_title(&windows, entry.source_window_title.as_deref(), deadline)?
    {
        return Ok(CandidateRoots {
            roots: vec![window],
            scope_verified: false,
        });
    }
    if bounded_window_fallback_allowed(entry) {
        let roots = fallback_replacement_window_roots(&windows, deadline)?;
        if !roots.is_empty() {
            return Ok(CandidateRoots {
                roots,
                scope_verified: false,
            });
        }
    }
    Ok(CandidateRoots {
        roots: Vec::new(),
        scope_verified: false,
    })
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
        _ => return Err(AdapterError::not_supported("snapshot surface")),
    };
    Ok(root)
}

#[cfg(target_os = "macos")]
fn exact_source_window_number_root(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(windows) = windows_for_pid(entry.pid, deadline)? else {
        return Ok(None);
    };
    window_by_number(&windows, source_window_number(entry), deadline)
}

#[cfg(target_os = "macos")]
fn exact_source_window_root(
    entry: &RefEntry,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(windows) = windows_for_pid(entry.pid, deadline)? else {
        return Ok(None);
    };
    exact_source_window_from_windows(&windows, entry, deadline)
}

#[cfg(target_os = "macos")]
fn exact_source_window_from_windows(
    windows: &[AXElement],
    entry: &RefEntry,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    if let Some(window) = window_by_number(windows, source_window_number(entry), deadline)? {
        return Ok(Some(window));
    }
    window_by_title(windows, entry.source_window_title.as_deref(), deadline)
}

#[cfg(target_os = "macos")]
fn windows_for_pid(pid: i32, deadline: Instant) -> Result<Option<Vec<AXElement>>, AdapterError> {
    let root = element_for_pid(pid);
    prepare_for_read(&root, deadline)?;
    Ok(copy_ax_array(&root, "AXWindows"))
}

#[cfg(target_os = "macos")]
fn window_by_number(
    windows: &[AXElement],
    source_window_number: Option<i64>,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(source_window_number) = source_window_number else {
        return Ok(None);
    };
    for win in windows {
        prepare_for_read(win, deadline)?;
        if copy_i64_attr(win, "AXWindowNumber") == Some(source_window_number) {
            return Ok(Some(win.clone()));
        }
    }
    Ok(None)
}

#[cfg(target_os = "macos")]
fn window_by_title(
    windows: &[AXElement],
    source_window_title: Option<&str>,
    deadline: Instant,
) -> Result<Option<AXElement>, AdapterError> {
    let Some(source_window_title) = source_window_title else {
        return Ok(None);
    };
    let mut found = None;
    for win in windows {
        prepare_for_read(win, deadline)?;
        if copy_string_attr(win, "AXTitle").as_deref() == Some(source_window_title) {
            if found.is_some() {
                return Ok(None);
            }
            found = Some(win.clone());
        }
    }
    Ok(found)
}

#[cfg(target_os = "macos")]
pub(super) fn fallback_replacement_window_roots(
    windows: &[AXElement],
    deadline: Instant,
) -> Result<Vec<AXElement>, AdapterError> {
    let mut roots = Vec::new();
    let mut dedupe = ElementDedupe;
    for win in windows {
        prepare_for_read(win, deadline)?;
        dedupe.push(&mut roots, win.clone());
    }
    Ok(roots)
}

#[cfg(target_os = "macos")]
pub(super) fn source_window_scope_required(entry: &RefEntry) -> bool {
    matches!(entry.source_surface, SnapshotSurface::Window) && source_window_number(entry).is_some()
}

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
