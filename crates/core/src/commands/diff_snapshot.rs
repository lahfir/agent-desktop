use crate::{
    adapter::{PlatformAdapter, SnapshotSurface},
    diff,
    error::AppError,
    snapshot,
    store,
};
use serde_json::{json, Value};

pub struct DiffSnapshotArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
    pub text: bool,
}

pub fn execute(args: DiffSnapshotArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let previous = store::load().ok().flatten();

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
    let win = &result.window;

    let Some(prev) = previous else {
        let tree_val = serde_json::to_value(&result.tree)?;
        return Ok(json!({
            "app": win.app,
            "window": { "id": win.id, "title": win.title },
            "ref_count": ref_count,
            "no_baseline": true,
            "message": "No previous snapshot found. Run 'snapshot' or 'diff-snapshot' again after making UI changes to see a diff.",
            "tree": tree_val
        }));
    };

    let (entries, stats) = diff::diff_trees(&prev.tree, &result.tree);

    if args.text {
        let text = diff::format_text_diff(&entries, &stats);
        return Ok(json!({
            "app": win.app,
            "window": { "id": win.id, "title": win.title },
            "ref_count": ref_count,
            "from_app": prev.app,
            "from_window_title": prev.window_title,
            "from_snapshot_age_ms": snapshot_age_ms(prev.taken_at_ms),
            "changes": {
                "added": stats.added,
                "removed": stats.removed,
                "modified": stats.modified,
                "unchanged": stats.unchanged
            },
            "text": text
        }));
    }

    let entries_val = serde_json::to_value(&entries)?;

    Ok(json!({
        "app": win.app,
        "window": { "id": win.id, "title": win.title },
        "ref_count": ref_count,
        "from_app": prev.app,
        "from_window_title": prev.window_title,
        "from_snapshot_age_ms": snapshot_age_ms(prev.taken_at_ms),
        "changes": {
            "added": stats.added,
            "removed": stats.removed,
            "modified": stats.modified,
            "unchanged": stats.unchanged
        },
        "diff": entries_val
    }))
}

fn snapshot_age_ms(taken_at_ms: u64) -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    now.saturating_sub(taken_at_ms)
}
