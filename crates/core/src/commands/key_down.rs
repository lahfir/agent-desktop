use crate::{
    adapter::PlatformAdapter,
    commands::press::{check_blocked_combo, parse_combo},
    error::AppError,
};
use serde_json::{Value, json};

pub struct KeyDownArgs {
    pub combo: String,
}

pub fn execute(args: KeyDownArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    check_blocked_combo(&args.combo)?;
    let combo = parse_combo(&args.combo)?;
    adapter.key_event(&combo, true)?;
    Ok(json!({ "key_down": args.combo }))
}
