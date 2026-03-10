use crate::{
    adapter::{PlatformAdapter, SnapshotSurface},
    error::AppError,
    snapshot, snapshot_ref,
};
use serde_json::{json, Value};

pub struct SnapshotArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
    pub skeleton: bool,
    pub root_ref: Option<String>,
}

pub fn execute(args: SnapshotArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    tracing::debug!(
        "tree: snapshot app={:?} window_id={:?} max_depth={} interactive_only={} compact={}",
        args.app.as_deref().unwrap_or("(focused)"),
        args.window_id.as_deref().unwrap_or("(auto)"),
        args.max_depth,
        args.interactive_only,
        args.compact
    );

    let effective_depth = if args.skeleton {
        args.max_depth.min(3)
    } else {
        args.max_depth
    };

    let opts = crate::adapter::TreeOptions {
        max_depth: effective_depth,
        include_bounds: args.include_bounds,
        interactive_only: args.interactive_only,
        compact: args.compact,
        surface: args.surface,
        skeleton: args.skeleton,
        root_ref: args.root_ref.clone(),
    };

    if let Some(ref root) = args.root_ref {
        if !matches!(args.surface, SnapshotSurface::Window) {
            return Err(AppError::invalid_input(
                "--root cannot be combined with --surface",
            ));
        }
        return format_result(snapshot_ref::run_from_ref(adapter, &opts, root)?);
    }

    let result = snapshot::run(
        adapter,
        &opts,
        args.app.as_deref(),
        args.window_id.as_deref(),
    )?;

    format_result(result)
}

fn format_result(result: snapshot::SnapshotResult) -> Result<Value, AppError> {
    let ref_count = result.refmap.len();
    let tree = serde_json::to_value(&result.tree)?;
    let win = &result.window;

    tracing::debug!(
        "tree: snapshot complete app={:?} window={:?} refs={}",
        win.app,
        win.title,
        ref_count
    );

    Ok(json!({
        "app": win.app,
        "window": {
            "id": win.id,
            "title": win.title
        },
        "ref_count": ref_count,
        "tree": tree
    }))
}
