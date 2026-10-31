use crate::Commands;
use agent_desktop_core::AppError;
use std::io::{IsTerminal, Read};

const MAX_STDIN_BYTES: usize = 128 * 1024;

pub(crate) fn resolve_stdin_text(
    command: &mut Commands,
    reader: impl Read + IsTerminal,
) -> Result<(), AppError> {
    if let Some(value) = input(command) {
        if reader.is_terminal() {
            return Err(AppError::invalid_input(
                "--stdin requires a pipe or redirected file",
            ));
        }
        *value = Some(read_text(reader)?);
    }
    Ok(())
}

pub(crate) fn input(command: &mut Commands) -> Option<&mut Option<String>> {
    match command {
        Commands::Type(args) if args.stdin => Some(&mut args.text),
        Commands::SetValue(args) if args.stdin => Some(&mut args.value),
        Commands::ClipboardSet(args) if args.stdin => Some(&mut args.text),
        _ => None,
    }
}

pub(crate) fn redact_result(
    mut result: Result<serde_json::Value, AppError>,
    stdin: bool,
) -> Result<serde_json::Value, AppError> {
    if stdin {
        let data = match &mut result {
            Ok(data) => Some(data),
            Err(AppError::Adapter(error)) => error.details.as_mut(),
            _ => None,
        };
        if let Some(data) = data {
            omit_values(data);
        }
    }
    result
}

fn omit_values(data: &mut serde_json::Value) {
    match data {
        serde_json::Value::Object(fields) => {
            fields.remove("value");
            fields.values_mut().for_each(omit_values);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(omit_values),
        _ => {}
    }
}

pub(crate) fn read_text(reader: impl Read) -> Result<String, AppError> {
    let mut bytes = Vec::new();
    reader
        .take((MAX_STDIN_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| AppError::invalid_input("Could not read text from stdin"))?;
    validate_bytes(&bytes)?;
    String::from_utf8(bytes)
        .map_err(|_| AppError::invalid_input("stdin must contain valid UTF-8 text"))
}

pub(crate) fn validate_bytes(bytes: &[u8]) -> Result<(), AppError> {
    if bytes.len() > MAX_STDIN_BYTES {
        return Err(AppError::invalid_input(
            "stdin text exceeds the 128 KiB limit",
        ));
    }
    if bytes.contains(&0) {
        return Err(AppError::invalid_input(
            "stdin text cannot contain NUL bytes",
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "stdin_tests.rs"]
mod tests;
