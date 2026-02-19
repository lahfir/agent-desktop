use crate::{adapter::PlatformAdapter, error::AppError};
use serde::Deserialize;
use serde_json::{json, Value};

pub struct BatchArgs {
    pub commands_json: String,
    pub stop_on_error: bool,
}

#[derive(Debug, Deserialize)]
pub struct BatchCommand {
    pub command: String,
    #[serde(default)]
    pub args: Value,
}

pub fn parse_commands(json_str: &str) -> Result<Vec<BatchCommand>, AppError> {
    serde_json::from_str(json_str)
        .map_err(|e| AppError::invalid_input(format!("Invalid batch JSON: {e}")))
}

pub fn execute(args: BatchArgs, _adapter: &dyn PlatformAdapter) -> Result<Value, AppError> {
    let commands = parse_commands(&args.commands_json)?;
    Ok(json!({
        "note": "Batch execution delegated to dispatch layer",
        "count": commands.len()
    }))
}
