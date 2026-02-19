use crate::{adapter::PlatformAdapter, error::AppError};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct BatchArgs {
    pub commands_json: String,
    pub stop_on_error: bool,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct BatchCommand {
    command: String,
    #[serde(default)]
    args: Value,
}

pub fn execute(args: BatchArgs, _adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let commands: Vec<BatchCommand> = serde_json::from_str(&args.commands_json)
        .map_err(|e| AppError::invalid_input(format!("Invalid batch JSON: {e}")))?;

    Ok(json!({
        "note": "Batch dispatch is implemented in the binary crate dispatch layer",
        "count": commands.len()
    }))
}
