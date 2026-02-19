use agent_desktop_core::error::AdapterError;

#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use std::process::Command;

    pub fn get() -> Result<String, AdapterError> {
        let output = Command::new("pbpaste")
            .output()
            .map_err(|e| AdapterError::internal(format!("pbpaste failed: {e}")))?;
        String::from_utf8(output.stdout)
            .map_err(|_| AdapterError::internal("Clipboard contains non-UTF8 data"))
    }

    pub fn set(text: &str) -> Result<(), AdapterError> {
        use std::io::Write;
        let mut child = Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| AdapterError::internal(format!("pbcopy failed: {e}")))?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin
                .write_all(text.as_bytes())
                .map_err(|e| AdapterError::internal(format!("Write to pbcopy failed: {e}")))?;
        }
        child
            .wait()
            .map_err(|e| AdapterError::internal(format!("pbcopy wait failed: {e}")))?;
        Ok(())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::*;

    pub fn get() -> Result<String, AdapterError> {
        Err(AdapterError::not_supported("clipboard_get"))
    }

    pub fn set(_text: &str) -> Result<(), AdapterError> {
        Err(AdapterError::not_supported("clipboard_set"))
    }
}

pub use imp::{get, set};
