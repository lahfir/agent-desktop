use agent_desktop_core::{AdapterError, ErrorCode, session};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt};
use std::path::{Path, PathBuf};

/// Wire generation of the renderer protocol. Bump it whenever a control or
/// instruction gains a field, because renderers decode with
/// `deny_unknown_fields`; the version is hashed into every socket name, so a
/// new CLI never hands its controls to an older renderer.
pub(super) const PROTOCOL_VERSION: &str = "v3";

/// The generation this one replaces. Its renderers are retired with a Disable,
/// which every generation decodes, and are still reached by broadcast teardown.
const PREVIOUS_PROTOCOL_VERSION: &str = "v2";

pub(super) fn path(session_id: &str, agent_id: Option<&str>) -> Result<PathBuf, AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    let path = path_for_root(&root, session_id, agent_id);
    ensure_socket_parent(&path)?;
    validate_socket_path(&path)?;
    Ok(path)
}

pub(super) fn previous_generation_path(
    session_id: &str,
    agent_id: Option<&str>,
) -> Result<PathBuf, AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    let path = path_for_protocol(&root, session_id, agent_id, PREVIOUS_PROTOCOL_VERSION);
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
    path_for_protocol(root, session_id, agent_id, PROTOCOL_VERSION)
}

fn path_for_protocol(
    root: &Path,
    session_id: &str,
    agent_id: Option<&str>,
    protocol: &str,
) -> PathBuf {
    let name = match agent_id {
        Some(agent_id) => format!(
            "{}{:016x}.sock",
            agent_prefix(root, session_id, protocol),
            endpoint_hash(root, agent_id, Some("agent")),
        ),
        None => format!(
            ".cursor-overlay-{:016x}.sock",
            endpoint_hash(root, session_id, Some(protocol))
        ),
    };
    socket_for_name(root, name)
}

fn agent_prefix(root: &Path, session_id: &str, protocol: &str) -> String {
    format!(
        ".cursor-overlay-{:016x}-",
        endpoint_hash(root, session_id, Some(protocol))
    )
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
    match std::fs::symlink_metadata(path) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(
                AdapterError::internal("Could not verify the cursor overlay socket")
                    .with_platform_detail(error.to_string()),
            );
        }
    }
    if !is_private_socket(path) {
        return Err(AdapterError::internal(
            "Cursor overlay socket is not owned by the current user",
        ));
    }
    Ok(())
}

fn is_private_socket(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| {
        !metadata.file_type().is_symlink()
            && metadata.file_type().is_socket()
            && metadata.uid() == unsafe { libc::geteuid() }
            && metadata.mode() & 0o077 == 0
    })
}

pub(super) fn discover(
    session_id: &str,
) -> Result<(Vec<PathBuf>, Option<AdapterError>), AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    let prefixes = [PROTOCOL_VERSION, PREVIOUS_PROTOCOL_VERSION]
        .map(|protocol| agent_prefix(&root, session_id, protocol));
    let fallback = private_fallback_root();
    let expected = path_for_root(&root, session_id, None);
    ensure_socket_parent(&expected)?;
    let mut paths = [
        expected,
        previous_generation_path(session_id, None)?,
        legacy_path(session_id)?,
    ]
    .into_iter()
    .filter(|path| is_private_socket(path))
    .collect::<Vec<_>>();
    let needs_fallback =
        path_for_root(&root, session_id, Some("agent")).parent() == Some(fallback.as_path());
    let directories = if needs_fallback && validate_private_directory(&fallback)? {
        vec![root.clone(), fallback]
    } else {
        vec![root.clone()]
    };
    let (mut scanned, listing_error) = collect_session_sockets(&directories, &prefixes);
    paths.append(&mut scanned);
    paths.sort();
    paths.dedup();
    let listing_error = listing_error.map(|detail| {
        AdapterError::internal(
            "Cursor overlay socket directory could not be fully listed; teardown was not confirmed",
        )
        .with_platform_detail(detail)
    });
    Ok((paths, listing_error))
}

fn collect_session_sockets(
    directories: &[PathBuf],
    prefixes: &[String],
) -> (Vec<PathBuf>, Option<String>) {
    let mut paths = Vec::new();
    let mut listing_error: Option<String> = None;
    for directory in directories {
        let entries = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(error) => {
                tracing::warn!(path = %directory.display(), %error, "cursor overlay sockets could not be listed");
                if listing_error.is_none() {
                    listing_error = Some(error.to_string());
                }
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    tracing::warn!(path = %directory.display(), %error, "cursor overlay socket entry could not be read");
                    if listing_error.is_none() {
                        listing_error = Some(error.to_string());
                    }
                    continue;
                }
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let suffix = prefixes
                .iter()
                .find_map(|prefix| name.strip_prefix(prefix.as_str()))
                .and_then(|value| value.strip_suffix(".sock"));
            if suffix.is_some_and(|value| {
                value.len() == 16 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
            }) && is_private_socket(&entry.path())
            {
                paths.push(entry.path());
            }
        }
    }
    paths.sort();
    paths.dedup();
    (paths, listing_error)
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
#[path = "endpoint_tests.rs"]
mod tests;
