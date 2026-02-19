use crate::{
    action::{Action, Direction},
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref,
    error::AppError,
};
use serde_json::Value;

pub struct ScrollArgs {
    pub ref_id: String,
    pub direction: Direction,
    pub amount: u32,
}

pub fn execute(args: ScrollArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (_entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    let result = adapter.execute_action(&handle, Action::Scroll(args.direction, args.amount))?;
    Ok(serde_json::to_value(result)?)
}
