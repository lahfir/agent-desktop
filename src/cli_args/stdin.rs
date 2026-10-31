use crate::Commands;
use agent_desktop_core::AppError;
use std::io::Read;

const MAX_STDIN_BYTES: usize = 128 * 1024;

pub(crate) fn resolve_stdin_text(
    command: &mut Commands,
    reader: impl Read,
) -> Result<(), AppError> {
    let value = match command {
        Commands::Type(args) if args.stdin => Some(&mut args.text),
        Commands::SetValue(args) if args.stdin => Some(&mut args.value),
        Commands::ClipboardSet(args) if args.stdin => Some(&mut args.text),
        _ => None,
    };
    if let Some(value) = value {
        *value = Some(read_text(reader)?);
    }
    Ok(())
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
