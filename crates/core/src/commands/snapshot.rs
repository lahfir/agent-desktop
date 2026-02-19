use crate::{
    adapter::{PlatformAdapter, SnapshotSurface},
    error::AppError,
    snapshot,
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
}

pub fn execute(args: SnapshotArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let opts = crate::adapter::TreeOptions {
        max_depth: args.max_depth,
        include_bounds: args.include_bounds,
        interactive_only: args.interactive_only,
        compact: args.compact,
        surface: args.surface,
    };

    let result = snapshot::run(
        adapter,
        &opts,
        args.app.as_deref(),
        args.window_id.as_deref(),
    )?;

    let ref_count = result.refmap.len();
    let tree = serde_json::to_value(&result.tree)?;
    let win = &result.window;

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
