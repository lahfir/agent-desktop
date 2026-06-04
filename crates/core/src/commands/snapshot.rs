use crate::{
    adapter::{PlatformAdapter, SnapshotSurface},
    context::CommandContext,
    error::AppError,
    refs::validate_ref_id,
    snapshot, snapshot_ref,
};
use serde_json::{Value, json};

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
    pub snapshot_id: Option<String>,
}

fn tree_options(args: &SnapshotArgs) -> crate::adapter::TreeOptions {
    let skeleton_applies = args.skeleton && args.root_ref.is_none();
    let effective_depth = if skeleton_applies {
        args.max_depth.min(3)
    } else {
        args.max_depth
    };

    crate::adapter::TreeOptions {
        max_depth: effective_depth,
        include_bounds: args.include_bounds,
        interactive_only: args.interactive_only,
        compact: args.compact,
        surface: args.surface,
        skeleton: skeleton_applies,
    }
}

pub fn execute(args: SnapshotArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    execute_with_context(args, adapter, &CommandContext::default())
}

pub fn execute_with_context(
    args: SnapshotArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    tracing::debug!(
        "tree: snapshot app={:?} window_id={:?} max_depth={} interactive_only={} compact={}",
        args.app.as_deref().unwrap_or("(focused)"),
        args.window_id.as_deref().unwrap_or("(auto)"),
        args.max_depth,
        args.interactive_only,
        args.compact
    );

    let opts = tree_options(&args);

    if let Some(ref root) = args.root_ref {
        if !matches!(args.surface, SnapshotSurface::Window) {
            return Err(AppError::invalid_input(
                "--root cannot be combined with --surface",
            ));
        }
        validate_ref_id(root)?;
        return format_result(snapshot_ref::run_from_ref_with_context(
            adapter,
            &opts,
            root,
            args.snapshot_id.as_deref(),
            context,
        )?);
    }

    let result = snapshot::run_with_context(
        adapter,
        &opts,
        args.app.as_deref(),
        args.window_id.as_deref(),
        context,
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
        "snapshot_id": result.snapshot_id,
        "tree": tree
    }))
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
