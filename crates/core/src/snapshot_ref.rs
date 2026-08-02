use crate::{
    AppError,
    adapter::{PlatformAdapter, TreeOptions},
    context::CommandContext,
    live_locator::{ObservationRequest, ObservationRoot},
    ref_alloc::{self, RefAllocConfig},
    ref_alloc_options::RefAllocOptions,
    ref_alloc_scope::RefAllocScope,
    ref_alloc_source::RefAllocSource,
    refs_store::RefStore,
    snapshot::SnapshotResult,
};

#[cfg(test)]
pub fn run_from_ref(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    root_ref_id: &str,
    snapshot_id: Option<&str>,
) -> Result<SnapshotResult, AppError> {
    run_from_ref_with_context(
        adapter,
        opts,
        root_ref_id,
        snapshot_id,
        &CommandContext::default(),
    )
}

pub fn run_from_ref_with_context(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    root_ref_id: &str,
    snapshot_id: Option<&str>,
    context: &CommandContext,
) -> Result<SnapshotResult, AppError> {
    let store = RefStore::for_session(context.session_id())?;
    let (active_snapshot_id, local_root_ref) =
        crate::ref_token::resolve_ref_target(root_ref_id, snapshot_id)?;
    let refmap = store.load_snapshot(&active_snapshot_id)?;

    let entry = refmap
        .get(&local_root_ref)
        .ok_or_else(|| AppError::stale_ref(root_ref_id))?
        .clone();

    let deadline = crate::Deadline::after(3_000)?;
    let handle = adapter.resolve_element_strict(&entry, deadline)?;

    let observation_options = opts.with_ref_identity_bounds();
    let raw_tree = crate::renderer_accessibility::observe_tree(
        adapter,
        ObservationRoot::Element {
            handle: &handle,
            entry: &entry,
            root_ref: Some(&local_root_ref),
        },
        &ObservationRequest::snapshot(&observation_options, deadline).validate()?,
    )?
    .into_accessibility_tree()?;
    let nodes_observed = count_nodes(&raw_tree);

    let source_app = entry.source.source_app.as_deref();
    let source_window_id = entry.source.source_window_id.as_deref();
    let source_window_title = entry.source.source_window_title.as_deref();
    let path_prefix = entry.scope.path.clone();
    let config = RefAllocConfig {
        options: RefAllocOptions {
            include_bounds: opts.include_bounds,
            interactive_only: opts.interactive_only,
            compact: opts.compact,
        },
        source: RefAllocSource {
            pid: entry.process.pid,
            app: source_app,
            window_id: source_window_id,
            window_title: source_window_title,
            window_bounds_hash: entry.source.source_window_bounds_hash,
            process_instance: entry.process.process_instance.as_deref(),
            surface: entry.source.source_surface,
        },
        scope: RefAllocScope {
            root_ref_id: Some(&local_root_ref),
            path_prefix: path_prefix.as_slice(),
        },
    };

    let (mut tree, refmap) = store.update_existing_snapshot(
        &active_snapshot_id,
        &local_root_ref,
        &entry,
        |current| {
            current.remove_by_root_ref(&local_root_ref);
            let mut tree = ref_alloc::allocate_refs(raw_tree, current, &config)?;
            crate::hints::add_structural_hints(&mut tree);
            Ok(tree)
        },
    )?;
    crate::trace_artifacts::copy_refmap_if_full(context, &store, &active_snapshot_id, &refmap)?;
    crate::ref_token::qualify_tree_refs(&mut tree, &active_snapshot_id);
    context.trace_lazy("snapshot.root.saved", || {
        serde_json::json!({
            "root_ref": root_ref_id,
            "snapshot_id": active_snapshot_id,
            "ref_count": refmap.len()
        })
    })?;

    let instance = entry.process.process_instance.as_deref().ok_or_else(|| {
        AppError::Adapter(crate::AdapterError::stale_ref(
            "root ref has no process-instance identity",
        ))
    })?;
    let window = crate::window_lookup::find_window_for_process(
        crate::ProcessIdentity::new(entry.process.pid, instance),
        adapter,
        deadline,
    )?;

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
        snapshot_id: Some(active_snapshot_id),
        complete: true,
        nodes_observed,
    })
}

/// A drill-down replaces refs inside an existing snapshot, so it must observe
/// its whole subtree or leave the stored map untouched. Partial projection is
/// only safe for a full snapshot, which writes a fresh map and destroys nothing.
fn count_nodes(node: &crate::AccessibilityNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

#[cfg(test)]
#[path = "snapshot_ref_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "snapshot_ref_alloc_tests.rs"]
mod alloc_tests;
