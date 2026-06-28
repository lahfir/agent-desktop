use crate::{
    adapter::PlatformAdapter,
    commands::press::{check_blocked_combo, parse_combo},
    error::AppError,
};
use serde_json::{Value, json};

pub struct KeyUpArgs {
    pub combo: String,
}

pub fn execute(args: KeyUpArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    check_blocked_combo(&args.combo)?;
    let combo = parse_combo(&args.combo)?;
    adapter.key_event(&combo, false)?;
    Ok(json!({ "key_up": args.combo }))
}
