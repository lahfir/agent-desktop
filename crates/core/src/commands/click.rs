use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref,
    error::AppError,
};
use serde_json::Value;

pub struct ClickArgs {
    pub ref_id: String,
}

pub fn execute(args: ClickArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (_entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, Action::Click)?;
    Ok(serde_json::to_value(result)?)
}

pub fn execute_double(args: ClickArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (_entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, Action::DoubleClick)?;
    Ok(serde_json::to_value(result)?)
}

pub fn execute_right(args: ClickArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (_entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, Action::RightClick)?;
    Ok(serde_json::to_value(result)?)
}
