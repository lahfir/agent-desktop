use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::{resolve_ref, RefArgs},
    error::AppError,
};
use serde_json::Value;

pub fn execute(args: RefArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (_entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, Action::Collapse)?;
    Ok(serde_json::to_value(result)?)
}
