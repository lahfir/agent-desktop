use agent_desktop_core::{AdapterError, ErrorCode, session};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

pub(super) const PROTOCOL_VERSION: &str = "v2";

pub(super) fn path(session_id: &str, agent_id: Option<&str>) -> Result<PathBuf, AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    let path = path_for_root(&root, session_id, agent_id);
    ensure_socket_parent(&path)?;
    validate_socket_path(&path)?;
    Ok(path)
}

pub(super) fn legacy_path(session_id: &str) -> Result<PathBuf, AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    let path = socket_for_name(
        &root,
        format!(
            ".cursor-overlay-{:016x}.sock",
            endpoint_hash(&root, session_id, None)
        ),
    );
    ensure_socket_parent(&path)?;
    validate_socket_path(&path)?;
    Ok(path)
}

pub(super) fn lock_path() -> Result<PathBuf, AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    Ok(root.join(".cursor-overlay-start.lock"))
}

fn path_for_root(root: &Path, session_id: &str, agent_id: Option<&str>) -> PathBuf {
    let name = match agent_id {
        Some(agent_id) => format!(
            ".cursor-overlay-{:016x}-{:016x}.sock",
            endpoint_hash(root, session_id, Some(PROTOCOL_VERSION)),
            endpoint_hash(root, agent_id, Some("agent")),
        ),
        None => format!(
            ".cursor-overlay-{:016x}.sock",
            endpoint_hash(root, session_id, Some(PROTOCOL_VERSION))
        ),
    };
    socket_for_name(root, name)
}

fn socket_for_name(root: &Path, name: String) -> PathBuf {
    let path = root.join(&name);
    if path.as_os_str().as_bytes().len() < 100 {
        path
    } else {
        private_fallback_root().join(name)
    }
}

fn private_fallback_root() -> PathBuf {
    PathBuf::from("/tmp").join(format!("agent-desktop-{}", unsafe { libc::geteuid() }))
}

fn ensure_socket_parent(path: &Path) -> Result<(), AdapterError> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    if parent != private_fallback_root() {
        return Ok(());
    }
    ensure_private_directory(parent)
}

fn ensure_private_directory(parent: &Path) -> Result<(), AdapterError> {
    let mut builder = std::fs::DirBuilder::new();
    builder.mode(0o700);
    match builder.create(parent) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(AdapterError::internal(
                "Could not create the private cursor overlay socket directory",
            )
            .with_platform_detail(error.to_string()));
        }
    }
    if !validate_private_directory(parent)? {
        return Err(AdapterError::internal(
            "Cursor overlay socket directory is missing",
        ));
    }
    Ok(())
}

fn validate_private_directory(parent: &Path) -> Result<bool, AdapterError> {
    let metadata = match std::fs::symlink_metadata(parent) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => {
            return Err(AdapterError::internal(
                "Could not verify the cursor overlay socket directory",
            )
            .with_platform_detail(error.to_string()));
        }
    };
    if !metadata.file_type().is_dir()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(AdapterError::internal(
            "Cursor overlay socket directory is not private to the current user",
        ));
    }
    Ok(true)
}

fn validate_socket_path(path: &Path) -> Result<(), AdapterError> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(
                AdapterError::internal("Could not verify the cursor overlay socket")
                    .with_platform_detail(error.to_string()),
            );
        }
    };
    if metadata.file_type().is_symlink()
        || !metadata.file_type().is_socket()
        || metadata.uid() != unsafe { libc::geteuid() }
        || metadata.mode() & 0o077 != 0
    {
        return Err(AdapterError::internal(
            "Cursor overlay socket is not owned by the current user",
        ));
    }
    Ok(())
}

fn safe_socket_entry(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        metadata.file_type().is_socket()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o077 == 0
    })
}

pub(super) fn discover(session_id: &str) -> Result<Vec<PathBuf>, AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    let prefix = format!(
        ".cursor-overlay-{:016x}-",
        endpoint_hash(&root, session_id, Some(PROTOCOL_VERSION))
    );
    let fallback = private_fallback_root();
    let expected = path_for_root(&root, session_id, None);
    ensure_socket_parent(&expected)?;
    let mut paths = [expected, legacy_path(session_id)?]
        .into_iter()
        .filter(|path| safe_socket_entry(path))
        .collect::<Vec<_>>();
    let needs_fallback =
        path_for_root(&root, session_id, Some("agent")).parent() == Some(fallback.as_path());
    let directories = if needs_fallback && validate_private_directory(&fallback)? {
        vec![root.clone(), fallback]
    } else {
        vec![root.clone()]
    };
    for directory in directories {
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let suffix = name
                .strip_prefix(&prefix)
                .and_then(|value| value.strip_suffix(".sock"));
            if suffix.is_some_and(|value| {
                value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            }) && safe_socket_entry(&entry.path())
            {
                paths.push(entry.path());
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn endpoint_hash(root: &Path, session_id: &str, protocol: Option<&str>) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in root
        .as_os_str()
        .as_bytes()
        .iter()
        .chain(session_id.as_bytes())
        .chain(protocol.into_iter().flat_map(str::as_bytes))
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn long_state_root_uses_a_short_deterministic_socket_path() {
        let root = Path::new("/private/tmp").join("deep".repeat(40));
        let first = path_for_root(&root, "run-1", None);
        let second = path_for_root(&root, "run-1", None);

        assert_eq!(first, second);
        assert_eq!(first.parent(), Some(private_fallback_root().as_path()));
        assert!(first.as_os_str().as_bytes().len() < 100);
    }

    #[test]
    fn private_directory_is_owner_matched_and_mode_restricted() {
        let directory = std::env::temp_dir().join(format!(
            "agent-desktop-endpoint-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir(&directory);
        std::fs::create_dir(&directory).expect("create test directory");
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o755))
            .expect("make test directory permissive");
        assert!(ensure_private_directory(&directory).is_err());
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))
            .expect("protect test directory");

        ensure_private_directory(&directory).expect("restrict test directory");
        let metadata = std::fs::symlink_metadata(&directory).expect("read test directory");
        assert!(metadata.file_type().is_dir());
        assert_eq!(metadata.uid(), unsafe { libc::geteuid() });
        assert_eq!(metadata.mode() & 0o777, 0o700);
        std::fs::remove_dir(&directory).expect("remove test directory");
    }

    #[test]
    fn protocol_generation_uses_a_distinct_socket() {
        let root = Path::new("/private/tmp/state");

        assert_ne!(
            path_for_root(root, "run-1", None),
            path_for_root(root, "run-1", Some("agent-a"))
        );
    }

    #[test]
    fn named_endpoints_are_distinct_and_session_prefixed() {
        let root = Path::new("/private/tmp/state");
        let first = path_for_root(root, "run-1", Some("agent-a"));
        let second = path_for_root(root, "run-1", Some("agent-b"));
        assert_ne!(first, second);
        assert!(first.file_name().unwrap().to_string_lossy().contains('-'));
    }

    #[test]
    fn v2_is_a_valid_agent_id_without_colliding_with_default() {
        let root = Path::new("/private/tmp/state");
        assert_ne!(
            path_for_root(root, "run-1", None),
            path_for_root(root, "run-1", Some("v2"))
        );
    }
}
