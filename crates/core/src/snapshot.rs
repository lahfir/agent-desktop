use crate::{
    adapter::{PlatformAdapter, SnapshotSurface, TreeOptions, WindowFilter},
    error::AppError,
    node::{AccessibilityNode, WindowInfo},
    ref_alloc::{self, RefAllocConfig},
    refs::RefMap,
};

pub struct SnapshotResult {
    pub tree: AccessibilityNode,
    pub refmap: RefMap,
    pub window: WindowInfo,
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

    let raw_tree = adapter.get_tree(&window, opts)?;

    let mut refmap = RefMap::new();
    let config = RefAllocConfig {
        include_bounds: opts.include_bounds,
        interactive_only: opts.interactive_only,
        compact: opts.compact,
        pid: window.pid,
        source_app: Some(window.app.as_str()),
        root_ref_id: None,
    };
    let mut tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config);

    crate::hints::add_structural_hints(&mut tree);

    Ok(SnapshotResult {
        tree,
        refmap,
        window,
    })
}

pub fn run(
    adapter: &dyn PlatformAdapter,
    opts: &TreeOptions,
    app_name: Option<&str>,
    window_id: Option<&str>,
) -> Result<SnapshotResult, AppError> {
    let result = build(adapter, opts, app_name, window_id)?;
    result.refmap.save()?;
    Ok(result)
}

pub fn append_surface_refs(
    adapter: &dyn PlatformAdapter,
    pid: i32,
    source_app: Option<&str>,
    surface: SnapshotSurface,
) -> Option<AccessibilityNode> {
    let filter = WindowFilter {
        focused_only: false,
        app: None,
    };
    let windows = adapter.list_windows(&filter).ok()?;
    let window = windows.into_iter().find(|w| w.pid == pid)?;
    let opts = TreeOptions {
        surface,
        interactive_only: true,
        ..Default::default()
    };
    let raw_tree = adapter.get_tree(&window, &opts).ok()?;
    let mut refmap = RefMap::load().ok()?;
    let config = RefAllocConfig {
        include_bounds: false,
        interactive_only: true,
        compact: false,
        pid,
        source_app,
        root_ref_id: None,
    };
    let tree = ref_alloc::allocate_refs(raw_tree, &mut refmap, &config);
    refmap.save().ok()?;
    Some(tree)
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
