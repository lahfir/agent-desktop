use crate::{adapter::{PermissionStatus, PlatformAdapter}, error::AppError, refs::RefMap};
use serde_json::{json, Value};

pub fn execute(adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let permissions = match adapter.check_permissions() {
        PermissionStatus::Granted => json!({ "granted": true }),
        PermissionStatus::Denied { suggestion } => json!({
            "granted": false,
            "suggestion": suggestion
        }),
    };

    let ref_count = RefMap::load().ok().map(|m| m.len());

    Ok(json!({
        "platform": std::env::consts::OS,
        "version": env!("CARGO_PKG_VERSION"),
        "permissions": permissions,
        "ref_count": ref_count
    }))
}
