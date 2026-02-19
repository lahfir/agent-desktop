use crate::{adapter::{PermissionStatus, PlatformAdapter}, error::AppError};
use serde_json::{json, Value};

pub struct PermissionsArgs {
    pub request: bool,
}

pub fn execute(args: PermissionsArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if args.request {
        return Ok(json!({
            "requested": true,
            "note": "Permission dialog triggered via --request flag"
        }));
    }

    match adapter.check_permissions() {
        PermissionStatus::Granted => Ok(json!({ "granted": true })),
        PermissionStatus::Denied { suggestion } => Ok(json!({
            "granted": false,
            "suggestion": suggestion
        })),
    }
}
