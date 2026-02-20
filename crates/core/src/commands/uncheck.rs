use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref,
    error::AppError,
};
use serde_json::Value;

pub struct UncheckArgs {
    pub ref_id: String,
}

pub fn execute(args: UncheckArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (_entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, Action::Uncheck)?;
    Ok(serde_json::to_value(result)?)
}
