use agent_desktop_core::{AdapterError, ErrorCode};
use std::os::unix::fs::MetadataExt;

use super::clipboard_helper_protocol as protocol;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HelperIdentity {
    pub(crate) path: std::path::PathBuf,
    device: u64,
    inode: u64,
    owner: u32,
    mode: u32,
    links: u64,
}

impl HelperIdentity {
    pub(crate) fn discover() -> Result<Self, AdapterError> {
        if let Some(path) = std::env::var_os("AGENT_DESKTOP_MACOS_HELPER_PATH") {
            let path = std::path::PathBuf::from(path);
            if !path.is_absolute() {
                return Err(invalid("Explicit helper path must be absolute"));
            }
            return Self::read(path);
        }
        let mut candidates = Vec::new();
        if let Ok(executable) = std::env::current_exe()
            && let Some(parent) = executable.parent()
        {
            candidates.push(parent.join(protocol::HELPER_BASENAME));
        }
        if let Some(image) = super::clipboard_helper_dl::containing_image()
            && let Some(parent) = image.parent()
        {
            let candidate = parent.join(protocol::HELPER_BASENAME);
            if !candidates.contains(&candidate) {
                candidates.push(candidate);
            }
        }
        for candidate in candidates {
            match Self::read(candidate) {
                Ok(identity) => return Ok(identity),
                Err(error) if missing(&error) => {}
                Err(error) => return Err(error),
            }
        }
        Err(invalid("No colocated packaged helper was found"))
    }

    pub(crate) fn revalidate(&self) -> Result<(), AdapterError> {
        let current = Self::read(self.path.clone())?;
        if current == *self {
            Ok(())
        } else {
            Err(invalid("Helper filesystem identity changed during launch"))
        }
    }

    fn read(path: std::path::PathBuf) -> Result<Self, AdapterError> {
        if path.file_name().and_then(|name| name.to_str()) != Some(protocol::HELPER_BASENAME) {
            return Err(invalid("Helper path has the wrong packaged basename"));
        }
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            invalid_with_kind(
                format!("{}: {error}", path.display()),
                if error.kind() == std::io::ErrorKind::NotFound {
                    "clipboard_helper_not_found"
                } else {
                    "clipboard_helper_invalid"
                },
            )
        })?;
        let mode = metadata.mode();
        if !metadata.file_type().is_file()
            || metadata.uid() != unsafe { libc::geteuid() }
            || mode & 0o022 != 0
            || mode & 0o111 == 0
            || metadata.nlink() != 1
        {
            return Err(invalid(
                "Helper must be an owner-matched, executable, single-link regular file with no group/world writes",
            ));
        }
        Ok(Self {
            path,
            device: metadata.dev(),
            inode: metadata.ino(),
            owner: metadata.uid(),
            mode,
            links: metadata.nlink(),
        })
    }
}

fn invalid(detail: impl Into<String>) -> AdapterError {
    invalid_with_kind(detail, "clipboard_helper_invalid")
}

fn invalid_with_kind(detail: impl Into<String>, kind: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "The packaged macOS clipboard helper is missing or invalid",
    )
    .with_platform_detail(detail)
    .with_details(serde_json::json!({
        "kind": kind,
        "helper": protocol::HELPER_BASENAME,
    }))
    .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
}

fn missing(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("kind"))
        .and_then(serde_json::Value::as_str)
        == Some("clipboard_helper_not_found")
}
