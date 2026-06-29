use crate::{
    adapter::PlatformAdapter,
    commands::combo::{ensure_combo_allowed, parse_combo_normalized},
    error::AppError,
};
use serde_json::{Value, json};

pub struct KeyDownArgs {
    pub combo: String,
    pub force: bool,
}

pub fn execute(args: KeyDownArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let combo = parse_combo_normalized(&args.combo)?;
    ensure_combo_allowed(&combo, &args.combo, args.force, adapter)?;
    adapter.key_event(&combo, true)?;
    Ok(json!({ "key_down": args.combo }))
}
