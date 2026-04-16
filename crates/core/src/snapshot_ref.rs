use crate::{
    adapter::{PlatformAdapter, TreeOptions, WindowFilter},
    error::AppError,
    node::WindowInfo,
    ref_alloc::{self, RefAllocConfig},
    refs::RefMap,
    snapshot::SnapshotResult,
};

pub fn run_from_ref(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    root_ref_id: &str,
) -> Result<SnapshotResult, AppError> {
    let mut refmap = RefMap::load()?;

    let entry = refmap
        .get(root_ref_id)
        .ok_or_else(|| AppError::stale_ref(root_ref_id))?
        .clone();

    let handle = adapter.resolve_element(&entry)?;

    let raw_tree = adapter.get_subtree(&handle, opts)?;

    refmap.remove_by_root_ref(root_ref_id);

    let source_app = entry.source_app.as_deref();
    let config = RefAllocConfig {
        include_bounds: opts.include_bounds,
        interactive_only: opts.interactive_only,
        compact: opts.compact,
        pid: entry.pid,
        source_app,
        root_ref_id: Some(root_ref_id),
    };

    let mut tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config);

    crate::hints::add_structural_hints(&mut tree);

    refmap.save()?;

    let window = adapter
        .list_windows(&WindowFilter {
            focused_only: false,
            app: None,
        })
        .ok()
        .and_then(|ws| ws.into_iter().find(|w| w.pid == entry.pid))
        .unwrap_or(WindowInfo {
            id: String::new(),
            title: format!("subtree from {root_ref_id}"),
            app: entry.source_app.unwrap_or_default(),
            pid: entry.pid,
            bounds: None,
            is_focused: true,
        });

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
    })
}

#[cfg(test)]
#[path = "snapshot_ref_tests.rs"]
mod tests;
