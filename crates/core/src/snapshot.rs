use crate::{
    adapter::{PlatformAdapter, SnapshotSurface, TreeOptions, WindowFilter},
    context::CommandContext,
    error::AppError,
    node::{AccessibilityNode, WindowInfo},
    ref_alloc::{self, RefAllocConfig},
    refs::RefMap,
    refs_store::RefStore,
    trace_artifacts,
};
use serde_json::json;

#[derive(Clone)]
pub struct SnapshotResult {
    pub tree: AccessibilityNode,
    pub refmap: RefMap,
    pub window: WindowInfo,
    pub snapshot_id: Option<String>,
}

pub fn build(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    app_name: Option<&str>,
    window_id: Option<&str>,
) -> Result<SnapshotResult, AppError> {
    let filter = WindowFilter {
        focused_only: app_name.is_none() && window_id.is_none(),
        app: app_name.map(str::to_string),
    };

    let windows = adapter.list_windows(&filter)?;

    let window = if let Some(wid) = window_id {
        windows.into_iter().find(|w| w.id == wid).ok_or_else(|| {
            AppError::Adapter(
                crate::error::AdapterError::new(
                    crate::error::ErrorCode::WindowNotFound,
                    format!("No window with id {wid}"),
                )
                .with_suggestion("Run 'list-windows' to see available window IDs."),
            )
        })?
    } else if let Some(app) = app_name {
        windows
            .into_iter()
            .find(|w| w.app.eq_ignore_ascii_case(app) && w.is_focused)
            .or_else(|| {
                adapter
                    .list_windows(&WindowFilter {
                        focused_only: false,
                        app: Some(app.to_string()),
                    })
                    .ok()
                    .and_then(|ws| ws.into_iter().next())
            })
            .ok_or_else(|| {
                AppError::Adapter(
                    crate::error::AdapterError::new(
                        crate::error::ErrorCode::AppNotFound,
                        format!("No window found for app '{app}'"),
                    )
                    .with_suggestion(
                        "Verify the app is running. Use 'list-apps' to see running applications.",
                    ),
                )
            })?
    } else {
        windows.into_iter().find(|w| w.is_focused).ok_or_else(|| {
            AppError::Adapter(
                crate::error::AdapterError::new(
                    crate::error::ErrorCode::WindowNotFound,
                    "No focused window found",
                )
                .with_suggestion(
                    "Use --app to specify an application, or click a window to focus it.",
                ),
            )
        })?
    };

    let raw_tree = adapter.get_tree(&window, &opts.with_ref_identity_bounds())?;

    let mut refmap = RefMap::new();
    let config = RefAllocConfig {
        include_bounds: opts.include_bounds,
        interactive_only: opts.interactive_only,
        compact: opts.compact,
        pid: window.pid,
        source_app: Some(window.app.as_str()),
        source_window_id: Some(window.id.as_str()),
        source_window_title: Some(window.title.as_str()),
        source_surface: opts.surface,
        root_ref_id: None,
        path_prefix: &[],
    };
    let mut tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config);

    crate::hints::add_structural_hints(&mut tree);

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
        snapshot_id: None,
    })
}

#[cfg(test)]
pub fn run(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    app_name: Option<&str>,
    window_id: Option<&str>,
) -> Result<SnapshotResult, AppError> {
    run_with_context(
        adapter,
        opts,
        app_name,
        window_id,
        &CommandContext::default(),
    )
}

pub fn run_with_context(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    app_name: Option<&str>,
    window_id: Option<&str>,
    context: &CommandContext,
) -> Result<SnapshotResult, AppError> {
    let mut result = build(adapter, opts, app_name, window_id)?;
    let store = RefStore::for_session(context.session_id())?;
    let snapshot_id = store.save_new_snapshot(&result.refmap)?;
    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id)?;
    result.snapshot_id = Some(snapshot_id);
    emit_snapshot_saved(context, &result)?;
    Ok(result)
}

pub(crate) fn emit_snapshot_saved(
    context: &CommandContext,
    result: &SnapshotResult,
) -> Result<(), AppError> {
    context.trace_lazy("snapshot.saved", || {
        let mut fields = json!({
            "snapshot_id": result.snapshot_id,
            "ref_count": result.refmap.len(),
        });
        if !result.window.app.is_empty() {
            fields["app"] = json!(result.window.app);
        }
        fields
    })
}

pub fn append_surface_refs(
    adapter: &dyn PlatformAdapter,
    pid: i32,
    source_app: Option<&str>,
    surface: SnapshotSurface,
) -> Result<Option<AccessibilityNode>, AppError> {
    append_surface_refs_with_context(
        adapter,
        pid,
        source_app,
        surface,
        &CommandContext::default(),
    )
}

pub fn append_surface_refs_with_context(
    adapter: &dyn PlatformAdapter,
    pid: i32,
    source_app: Option<&str>,
    surface: SnapshotSurface,
    context: &CommandContext,
) -> Result<Option<AccessibilityNode>, AppError> {
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };
    let windows = adapter.list_windows(&filter)?;
    let Some(window) = windows.into_iter().find(|w| w.pid == pid) else {
        return Ok(None);
    };
    let opts = TreeOptions {
        surface,
        interactive_only: true,
        ..Default::default()
    };
    let raw_tree = adapter.get_tree(&window, &opts.with_ref_identity_bounds())?;
    let store = RefStore::for_session(context.session_id())?;
    let mut refmap = store.load_latest()?;
    let config = RefAllocConfig {
        include_bounds: false,
        interactive_only: true,
        compact: false,
        pid,
        source_app,
        source_window_id: Some(window.id.as_str()),
        source_window_title: Some(window.title.as_str()),
        source_surface: surface,
        root_ref_id: None,
        path_prefix: &[],
    };
    let tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config);
    if let Some(id) = store.latest_snapshot_id() {
        store.save_existing_snapshot(&id, &refmap)?;
        trace_artifacts::copy_refmap_if_full(context, &store, &id)?;
    } else {
        let id = store.save_new_snapshot(&refmap)?;
        trace_artifacts::copy_refmap_if_full(context, &store, &id)?;
    }
    Ok(Some(tree))
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
