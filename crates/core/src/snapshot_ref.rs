use crate::{
    adapter::{PlatformAdapter, TreeOptions},
    error::AppError,
    node::WindowInfo,
    ref_alloc::{self, RefAllocConfig},
    refs_store::RefStore,
    resolved_element::ResolvedElement,
    snapshot::SnapshotResult,
};

pub fn run_from_ref(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    root_ref_id: &str,
    snapshot_id: Option<&str>,
) -> Result<SnapshotResult, AppError> {
    let store = RefStore::new()?;
    let mut refmap = store.load(snapshot_id)?;
    let active_snapshot_id = snapshot_id
        .map(str::to_string)
        .or_else(|| store.latest_snapshot_id());

    let entry = refmap
        .get(root_ref_id)
        .ok_or_else(|| AppError::stale_ref(root_ref_id))?
        .clone();

    let handle = ResolvedElement::new(adapter, adapter.resolve_element_strict(&entry)?);

    let raw_tree = adapter.get_subtree(handle.handle(), opts)?;

    refmap.remove_by_root_ref(root_ref_id);

    let source_app = entry.source_app.as_deref();
    let source_window_id = entry.source_window_id.as_deref();
    let source_window_title = entry.source_window_title.as_deref();
    let path_prefix = entry.path.clone();
    let config = RefAllocConfig {
        include_bounds: opts.include_bounds,
        interactive_only: opts.interactive_only,
        compact: opts.compact,
        pid: entry.pid,
        source_app,
        source_window_id,
        source_window_title,
        source_surface: entry.source_surface,
        root_ref_id: Some(root_ref_id),
        path_prefix: path_prefix.as_slice(),
    };

    let mut tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config);

    crate::hints::add_structural_hints(&mut tree);

    let saved_snapshot_id = if let Some(id) = active_snapshot_id {
        store.save_existing_snapshot(&id, &refmap)?;
        Some(id)
    } else {
        Some(store.save_new_snapshot(&refmap)?)
    };

    let window =
        crate::window_lookup::find_window_for_pid(entry.pid, adapter).unwrap_or(WindowInfo {
            id: String::new(),
            title: entry
                .source_window_title
                .unwrap_or_else(|| format!("subtree from {root_ref_id}")),
            app: entry.source_app.unwrap_or_default(),
            pid: entry.pid,
            bounds: None,
            is_focused: true,
        });

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
        snapshot_id: saved_snapshot_id,
    })
}

#[cfg(test)]
#[path = "snapshot_ref_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "snapshot_ref_alloc_tests.rs"]
mod alloc_tests;
