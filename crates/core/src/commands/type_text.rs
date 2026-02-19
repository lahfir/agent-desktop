use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::helpers::resolve_ref,
    error::AppError,
};
use serde_json::Value;

const MAX_TEXT_LEN: usize = 10_000;

pub struct TypeArgs {
    pub ref_id: String,
    pub text: String,
}

pub fn execute(args: TypeArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    if args.text.len() > MAX_TEXT_LEN {
        return Err(AppError::invalid_input(format!(
            "Text exceeds maximum length of {MAX_TEXT_LEN} characters"
        )));
    }

    let (_entry, handle) = resolve_ref(&args.ref_id, adapter)?;
    adapter.execute_action(&handle, Action::SetFocus)?;
    let result = adapter.execute_action(&handle, Action::TypeText(args.text))?;
    Ok(serde_json::to_value(result)?)
}
