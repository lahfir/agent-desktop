use crate::{
    action::{Action, ActionRequest, Direction},
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref,
    error::AppError,
};
use serde_json::Value;

pub struct ScrollArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub direction: Direction,
    pub amount: u32,
}

pub fn execute(args: ScrollArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let (entry, handle) = resolve_ref(&args.ref_id, args.snapshot_id.as_deref(), adapter)?;
    let request = ActionRequest::headless(Action::Scroll(args.direction, args.amount));
    crate::actionability::check(&entry, &request)?;
    let result = adapter.execute_action(handle.handle(), request)?;
    Ok(serde_json::to_value(result)?)
}
