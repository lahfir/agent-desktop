use crate::{
    action::Action, adapter::PlatformAdapter, commands::press::parse_combo, error::AppError,
};
use serde_json::{json, Value};

pub struct KeyUpArgs {
    pub combo: String,
}

pub fn execute(args: KeyUpArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let combo = parse_combo(&args.combo)?;
    let handle = crate::adapter::NativeHandle::null();
    adapter.execute_action(&handle, Action::KeyUp(combo))?;
    Ok(json!({ "key_up": args.combo }))
}
