use crate::{
    AccessibilityNode, AppError, WindowInfo,
    adapter::{PlatformAdapter, TreeOptions, WindowFilter},
    context::CommandContext,
    live_locator::{ObservationRequest, ObservationRoot},
    ref_alloc::{self, RefAllocConfig},
    ref_alloc_options::RefAllocOptions,
    ref_alloc_scope::RefAllocScope,
    ref_alloc_source::RefAllocSource,
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
    pub complete: bool,
    pub nodes_observed: usize,
}

impl SnapshotResult {
    pub(crate) fn bind_snapshot_id(&mut self, snapshot_id: String) {
        crate::ref_token::qualify_tree_refs(&mut self.tree, &snapshot_id);
        self.snapshot_id = Some(snapshot_id);
    }
}

pub fn build(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    app_name: Option<&str>,
    window_id: Option<&str>,
    deadline: crate::Deadline,
) -> Result<SnapshotResult, AppError> {
    let window = resolve_window(adapter, app_name, window_id, deadline)?;
    let observation_options = opts.with_ref_identity_bounds();
    let (raw_tree, complete, nodes_observed) = crate::renderer_accessibility::observe_tree(
        adapter,
        ObservationRoot::Window(&window),
        &ObservationRequest::snapshot(&observation_options, deadline).validate()?,
    )?
    .into_accessibility_tree_partial()?;

    let mut refmap = RefMap::new();
    let config = RefAllocConfig {
        options: RefAllocOptions {
            include_bounds: opts.include_bounds,
            interactive_only: opts.interactive_only,
            compact: opts.compact,
        },
        source: RefAllocSource {
            pid: window.pid,
            app: Some(window.app.as_str()),
            window_id: Some(window.id.as_str()),
            window_title: Some(window.title.as_str()),
            window_bounds_hash: window.bounds.as_ref().and_then(crate::Rect::bounds_hash),
            process_instance: window.process_instance.as_deref(),
            surface: opts.surface,
        },
        scope: RefAllocScope {
            root_ref_id: None,
            path_prefix: &[],
        },
    };
    let mut tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config)?;

    crate::hints::add_structural_hints(&mut tree);

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
        snapshot_id: None,
        complete,
        nodes_observed,
    })
}

pub(crate) fn resolve_window(
    adapter: &dyn PlatformAdapter,
    app_name: Option<&str>,
    window_id: Option<&str>,
    deadline: crate::Deadline,
) -> Result<WindowInfo, AppError> {
    let filter = WindowFilter {
        focused_only: app_name.is_none() && window_id.is_none(),
        app: app_name.map(str::to_string),
    };

    let windows = adapter.list_windows(&filter, deadline)?;

    if let Some(wid) = window_id {
        windows.into_iter().find(|w| w.id == wid).ok_or_else(|| {
            AppError::Adapter(
                crate::AdapterError::new(
                    crate::ErrorCode::WindowNotFound,
                    format!("No window with id {wid}"),
                )
                .with_suggestion("Run 'list-windows' to see available window IDs."),
            )
        })
    } else if let Some(app) = app_name {
        let candidates = windows
            .into_iter()
            .filter(|window| window.app.eq_ignore_ascii_case(app))
            .collect::<Vec<_>>();
        crate::window_lookup::select_window(
            candidates,
            crate::AdapterError::new(
                crate::ErrorCode::AppNotFound,
                format!("No window found for app '{app}'"),
            ),
            "More than one window matches the target",
        )
    } else {
        crate::window_lookup::select_window(
            windows,
            crate::AdapterError::new(crate::ErrorCode::WindowNotFound, "No focused window found"),
            "More than one window matches the target",
        )
    }
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
        &SnapshotTarget {
            app_name,
            window_id,
        },
        &CommandContext::default(),
        DEFAULT_SNAPSHOT_TIMEOUT_MS,
    )
}

/// Which window a full snapshot targets: the focused window when both fields
/// are `None`, a named app's focused window, or an exact window id.
pub struct SnapshotTarget<'a> {
    pub app_name: Option<&'a str>,
    pub window_id: Option<&'a str>,
}

/// The default snapshot observation deadline.
///
/// A16-11 measured a cold Chromium settle at 10-25 s against the previous
/// hardcoded 3 s, so callers can raise it explicitly via `--timeout-ms`; the
/// default stays 3 s for the ecosystems that settle fast.
pub const DEFAULT_SNAPSHOT_TIMEOUT_MS: u64 = 3_000;

/// Runs a full snapshot for `target` and persists its refmap under
/// `context`'s session, using `timeout_ms` as the observation deadline
/// (`DEFAULT_SNAPSHOT_TIMEOUT_MS` for callers with no reason to raise it).
pub fn run_with_context(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    target: &SnapshotTarget,
    context: &CommandContext,
    timeout_ms: u64,
) -> Result<SnapshotResult, AppError> {
    let mut result = build(
        adapter,
        opts,
        target.app_name,
        target.window_id,
        crate::Deadline::after(timeout_ms)?,
    )?;
    let store = RefStore::for_session(context.session_id())?;
    let snapshot_id = store.save_new_snapshot(&result.refmap)?;
    trace_artifacts::copy_refmap_if_full(context, &store, &snapshot_id, &result.refmap)?;
    result.bind_snapshot_id(snapshot_id);
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

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
