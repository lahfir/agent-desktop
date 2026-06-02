use agent_desktop_core::{adapter::SnapshotSurface, refs::RefEntry};
use rustc_hash::FxHashSet;

use super::AXElement;
use super::builder::window_element_for;
use super::element::{
    copy_ax_array, copy_element_attr, copy_i64_attr, copy_string_attr, element_for_pid,
};

#[cfg(target_os = "macos")]
pub(super) fn path_candidate_roots(entry: &RefEntry) -> Vec<AXElement> {
    if entry.bounds_hash.is_some() {
        return candidate_roots(entry);
    }
    scoped_surface_root(entry).into_iter().collect()
}

#[cfg(target_os = "macos")]
pub(super) fn candidate_roots(entry: &RefEntry) -> Vec<AXElement> {
    let root = element_for_pid(entry.pid);
    let mut roots = Vec::new();
    let mut seen = FxHashSet::default();
    if let Some(source_window_title) = entry.source_window_title.as_deref() {
        push_unique_root(
            &mut roots,
            &mut seen,
            window_element_for(entry.pid, source_window_title),
        );
    }
    if let Some(focused) = copy_element_attr(&root, "AXFocusedWindow") {
        push_unique_root(&mut roots, &mut seen, focused);
    }
    if let Some(main) = copy_element_attr(&root, "AXMainWindow") {
        push_unique_root(&mut roots, &mut seen, main);
    }
    for window in copy_ax_array(&root, "AXWindows").unwrap_or_default() {
        push_unique_root(&mut roots, &mut seen, window);
    }
    if let Some(menubar) = crate::tree::menubar_for_pid(entry.pid) {
        push_unique_root(&mut roots, &mut seen, menubar);
    }
    if let Some(menu) = crate::tree::menu_element_for_pid(entry.pid) {
        push_unique_root(&mut roots, &mut seen, menu);
    }
    if roots.is_empty() {
        roots.push(root);
    }
    roots
}

#[cfg(target_os = "macos")]
fn scoped_surface_root(entry: &RefEntry) -> Option<AXElement> {
    match entry.source_surface {
        SnapshotSurface::Window => exact_source_window_root(entry),
        SnapshotSurface::Focused => crate::tree::focused_surface_for_pid(entry.pid),
        SnapshotSurface::Menu => crate::tree::menu_element_for_pid(entry.pid),
        SnapshotSurface::Menubar => crate::tree::menubar_for_pid(entry.pid),
        SnapshotSurface::Sheet => crate::tree::sheet_for_pid(entry.pid),
        SnapshotSurface::Popover => crate::tree::popover_for_pid(entry.pid),
        SnapshotSurface::Alert => crate::tree::alert_for_pid(entry.pid),
    }
}

#[cfg(target_os = "macos")]
fn exact_source_window_root(entry: &RefEntry) -> Option<AXElement> {
    let root = element_for_pid(entry.pid);
    let windows = copy_ax_array(&root, "AXWindows")?;
    if let Some(source_window_number) = source_window_number(entry) {
        if let Some(window) = windows
            .iter()
            .find(|win| copy_i64_attr(win, "AXWindowNumber") == Some(source_window_number))
        {
            return Some(window.clone());
        }
    }
    let source_window_title = entry.source_window_title.as_deref()?;
    windows
        .into_iter()
        .find(|win| copy_string_attr(win, "AXTitle").as_deref() == Some(source_window_title))
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
fn push_unique_root(roots: &mut Vec<AXElement>, seen: &mut FxHashSet<usize>, root: AXElement) {
    if seen.insert(root.0 as usize) {
        roots.push(root);
    }
}
