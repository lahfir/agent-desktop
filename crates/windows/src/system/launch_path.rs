use agent_desktop_core::{AdapterError, ErrorCode, launch_options::LaunchOptions};
use std::path::PathBuf;

const MAX_ARGUMENT_COUNT: usize = 256;
const MAX_ENVIRONMENT_COUNT: usize = 256;
const MAX_LAUNCH_TEXT_BYTES: usize = 1024 * 1024;

pub(super) fn validate_app_identifier(id: &str) -> Result<(), AdapterError> {
    if id.is_empty() || id.contains("..") || id.chars().any(char::is_control) {
        return Err(invalid_identifier(id));
    }
    if is_absolute_launch_path(id) || is_bare_name(id) {
        return Ok(());
    }
    Err(invalid_identifier(id))
}

pub(super) fn validate_launch_options(options: &LaunchOptions) -> Result<(), AdapterError> {
    if options.args.len() > MAX_ARGUMENT_COUNT || options.env.len() > MAX_ENVIRONMENT_COUNT {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch argument or environment entry count exceeds the supported limit",
        ));
    }
    let text_bytes = options
        .args
        .iter()
        .map(String::len)
        .chain(
            options
                .env
                .iter()
                .map(|(key, value)| key.len() + value.len()),
        )
        .try_fold(0_usize, usize::checked_add)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Launch options are too large"))?;
    if text_bytes > MAX_LAUNCH_TEXT_BYTES {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Launch argument and environment data exceeds one MiB",
        ));
    }
    Ok(())
}

pub(crate) fn resolve_executable(id: &str) -> Result<PathBuf, AdapterError> {
    if is_absolute_launch_path(id) {
        let path = PathBuf::from(id);
        if path.is_file() {
            return Ok(path);
        }
        return Err(
            AdapterError::new(ErrorCode::AppNotFound, "Launch path does not exist")
                .with_details(serde_json::json!({ "app_name": id })),
        );
    }
    if !is_bare_name(id) {
        return Err(invalid_identifier(id));
    }
    let (system32, windows) = system_directories()?;
    for directory in [system32, windows] {
        let candidate = directory.join(id);
        if candidate.is_file() {
            return Ok(candidate);
        }
    }
    Err(AdapterError::new(
        ErrorCode::AppNotFound,
        "Bare launch name is not present in the system directories",
    )
    .with_details(serde_json::json!({ "app_name": id }))
    .with_suggestion(
        "Pass an absolute path, or a bare name that exists under System32 or the Windows directory.",
    ))
}

pub(super) fn image_file_name(id: &str) -> &str {
    id.rsplit(['\\', '/']).next().unwrap_or(id)
}

fn system_directories() -> Result<(PathBuf, PathBuf), AdapterError> {
    let root = std::env::var_os("SystemRoot")
        .or_else(|| std::env::var_os("WINDIR"))
        .ok_or_else(|| AdapterError::internal("SystemRoot is unset"))?;
    let windows = PathBuf::from(root);
    let system32 = windows.join("System32");
    Ok((system32, windows))
}

fn is_absolute_launch_path(id: &str) -> bool {
    if id.starts_with(r"\\?\") || id.starts_with(r"\\") {
        return true;
    }
    let bytes = id.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\'
}

fn is_bare_name(id: &str) -> bool {
    if id.is_empty() || id.contains(['\\', '/']) {
        return false;
    }
    let bytes = id.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }
    true
}

fn invalid_identifier(id: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::InvalidArgs,
        "Invalid app identifier: use an absolute path or a bare executable name",
    )
    .with_details(serde_json::json!({ "app_name": id }))
    .with_suggestion(
        "Use a full path (drive + backslash, UNC, or \\\\?\\) or a bare name resolved only under System32 / Windows.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_and_bare_names_are_accepted() {
        assert!(validate_app_identifier(r"C:\Windows\System32\notepad.exe").is_ok());
        assert!(validate_app_identifier(r"\\?\C:\Windows\notepad.exe").is_ok());
        assert!(validate_app_identifier(r"\\server\share\app.exe").is_ok());
        assert!(validate_app_identifier("notepad.exe").is_ok());
    }

    #[test]
    fn relative_and_unsafe_identifiers_are_rejected() {
        for identifier in [
            "",
            "..\\evil.exe",
            "sub\\app.exe",
            ".\\app.exe",
            "\\app.exe",
            "C:app.exe",
            "bad\0name",
            "bad\nname",
        ] {
            let error = validate_app_identifier(identifier).expect_err("unsafe identifier");
            assert_eq!(error.code, ErrorCode::InvalidArgs);
        }
    }
}
