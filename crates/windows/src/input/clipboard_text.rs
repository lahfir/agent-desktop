//! Pure `CF_UNICODETEXT` marshalling: NUL-terminated UTF-16LE ↔ `String`.

use agent_desktop_core::{AdapterError, ErrorCode};

const MAX_CLIPBOARD_TEXT_UTF16: usize = 1_000_000;

pub(crate) fn decode_utf16_text(bytes: &[u8]) -> Result<String, AdapterError> {
    if bytes.len() % 2 != 0 {
        return Err(payload_error(
            "CF_UNICODETEXT payload length is not a whole number of UTF-16 units",
        ));
    }
    let units = read_utf16_units(bytes);
    let end = units
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(units.len());
    if end > MAX_CLIPBOARD_TEXT_UTF16 {
        return Err(payload_error(
            "CF_UNICODETEXT exceeds the supported resource budget",
        ));
    }
    Ok(String::from_utf16_lossy(&units[..end]))
}

pub(crate) fn encode_utf16_text(text: &str) -> Result<Vec<u8>, AdapterError> {
    if text.contains('\0') {
        return Err(argument_error(
            "Clipboard text must not contain an embedded NUL",
        ));
    }
    let unit_count = text.encode_utf16().count();
    if unit_count > MAX_CLIPBOARD_TEXT_UTF16 {
        return Err(argument_error(
            "clipboard text exceeds the supported resource budget",
        ));
    }
    let mut out = Vec::with_capacity((unit_count + 1) * 2);
    for unit in text.encode_utf16() {
        out.extend_from_slice(&unit.to_le_bytes());
    }
    out.extend_from_slice(&0u16.to_le_bytes());
    Ok(out)
}

fn read_utf16_units(bytes: &[u8]) -> Vec<u16> {
    bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect()
}

fn payload_error(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, message)
}

fn argument_error(message: &str) -> AdapterError {
    AdapterError::new(ErrorCode::InvalidArgs, message)
}

#[cfg(test)]
#[path = "clipboard_text_tests.rs"]
mod tests;
