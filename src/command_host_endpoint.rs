use agent_desktop_core::AppError;
use std::hash::{Hash, Hasher};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

pub(super) fn identity(session: Option<&str>) -> Result<String, AppError> {
    let executable = std::env::current_exe()?;
    let metadata = std::fs::metadata(&executable)?;
    Ok(serde_json::to_string(&serde_json::json!({
        "protocol": 1,
        "root": agent_desktop_core::session::agent_desktop_dir()?,
        "session": session,
        "executable": executable,
        "revision": [metadata.dev(), metadata.ino(), metadata.size()],
        "modified": [metadata.mtime(), metadata.mtime_nsec()],
        "helper": std::env::var_os("AGENT_DESKTOP_MACOS_HELPER_PATH"),
    }))?)
}

pub(super) fn path(identity: &str) -> Result<PathBuf, AppError> {
    let directory =
        PathBuf::from("/tmp").join(format!("agent-desktop-host-{}", unsafe { libc::geteuid() }));
    match std::fs::DirBuilder::new().mode(0o700).create(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    validate_directory(&directory)?;
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    identity.hash(&mut hash);
    Ok(directory.join(format!("{:016x}.sock", hash.finish())))
}

pub(super) fn validate_directory(path: &Path) -> Result<(), AppError> {
    let metadata = std::fs::symlink_metadata(path)?;
    if !metadata.is_dir()
        || metadata.file_type().is_symlink()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(AppError::invalid_input(
            "Command host directory must be private to the current user",
        ));
    }
    Ok(())
}

pub(super) fn validate_socket(path: &Path) -> Result<bool, AppError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if !metadata.file_type().is_socket()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(AppError::invalid_input(
            "Command host endpoint is not a private socket",
        ));
    }
    Ok(true)
}
