use crate::{adapter::PlatformAdapter, error::AppError};
use serde_json::{json, Value};

pub struct DismissAllNotificationsArgs {
    pub app: Option<String>,
}

pub fn execute(
    args: DismissAllNotificationsArgs,
    adapter: &dyn PlatformAdapter,
) -> Result<Value, AppError> {
    let (dismissed, failures) = adapter.dismiss_all_notifications(args.app.as_deref())?;
    let mut result = json!({
        "dismissed_count": dismissed.len(),
    });
    if !failures.is_empty() {
        result["failures"] = json!(failures);
        result["failed_count"] = json!(failures.len());
    }
    Ok(result)
}
