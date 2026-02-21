use crate::{
    action::Action,
    adapter::{PlatformAdapter, SnapshotSurface},
    commands::helpers::resolve_ref,
    error::AppError,
    snapshot,
};
use serde_json::Value;

pub struct RightClickArgs {
    pub ref_id: String,
}

pub fn execute(args: RightClickArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, Action::RightClick)?;
    let mut response = serde_json::to_value(&result)?;

    std::thread::sleep(std::time::Duration::from_millis(250));

    if let Some(menu_tree) = snapshot::append_surface_refs(
        adapter,
        entry.pid,
        entry.source_app.as_deref(),
        SnapshotSurface::Menu,
    ) {
        if let Ok(menu_json) = serde_json::to_value(&menu_tree) {
            response["menu"] = menu_json;
        }
    }

    Ok(response)
}
