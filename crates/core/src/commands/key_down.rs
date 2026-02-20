use crate::{
    action::Action,
    adapter::PlatformAdapter,
    commands::press::parse_combo,
    error::AppError,
};
use serde_json::{json, Value};

pub struct KeyDownArgs {
    pub combo: String,
}

pub fn execute(args: KeyDownArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let combo = parse_combo(&args.combo)?;
    let handle = crate::adapter::NativeHandle::null();
    adapter.execute_action(&handle, Action::KeyDown(combo))?;
    Ok(json!({ "key_down": args.combo }))
}
