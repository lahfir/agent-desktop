use agent_desktop_core::{AdapterError, CursorOverlayInstruction, ErrorCode};
use std::io::Write;
use std::process::{Command, Stdio};

use super::child::{MARKER, PROTOCOL_VERSION};

const MAX_INSTRUCTION_BYTES: usize = 4 * 1024;

pub(crate) fn present(instruction: &CursorOverlayInstruction) -> Result<(), AdapterError> {
    let executable = std::env::current_exe().map_err(|error| {
        AdapterError::internal("Could not locate the cursor overlay executable")
            .with_platform_detail(error.to_string())
    })?;
    if executable.file_stem().and_then(|name| name.to_str()) != Some("agent-desktop") {
        return Ok(());
    }
    let payload = serde_json::to_vec(instruction).map_err(|error| {
        AdapterError::internal("Could not encode macOS cursor overlay instruction")
            .with_platform_detail(error.to_string())
    })?;
    if payload.len() > MAX_INSTRUCTION_BYTES {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Cursor overlay instruction exceeds the transport limit",
        ));
    }
    let mut child = Command::new(executable)
        .env(MARKER, PROTOCOL_VERSION)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            AdapterError::internal("Could not start the macOS cursor overlay")
                .with_platform_detail(error.to_string())
        })?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        AdapterError::internal("Cursor overlay child did not expose its input pipe")
    })?;
    stdin.write_all(&payload).map_err(|error| {
        AdapterError::internal("Could not send the cursor overlay instruction")
            .with_platform_detail(error.to_string())
    })?;
    Ok(())
}
