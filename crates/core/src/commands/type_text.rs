use crate::{
    action::{Action, ActionRequest},
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref,
    error::AppError,
};
use serde_json::Value;

const MAX_TEXT_LEN: usize = 10_000;

pub struct TypeArgs {
    pub ref_id: String,
    pub snapshot_id: Option<String>,
    pub text: String,
}

pub fn execute(args: TypeArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if args.text.len() > MAX_TEXT_LEN {
        return Err(AppError::invalid_input(format!(
            "Text exceeds maximum length of {MAX_TEXT_LEN} characters"
        )));
    }

    let (entry, handle) = resolve_ref(&args.ref_id, args.snapshot_id.as_deref(), adapter)?;
    let request = ActionRequest::focus_fallback(Action::TypeText(args.text));
    crate::actionability::check(&entry, &request)?;
    let result = adapter.execute_action(handle.handle(), request)?;
    Ok(serde_json::to_value(result)?)
}
