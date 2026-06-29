use crate::{
    adapter::PlatformAdapter,
    commands::combo::{ensure_combo_allowed, parse_combo_normalized},
    error::AppError,
};
use serde_json::{Value, json};

pub struct KeyUpArgs {
    pub combo: String,
    pub force: bool,
}

pub fn execute(args: KeyUpArgs, adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let combo = parse_combo_normalized(&args.combo)?;
    ensure_combo_allowed(&combo, &args.combo, args.force, adapter)?;
    adapter.key_event(&combo, false)?;
    Ok(json!({ "key_up": args.combo }))
}
