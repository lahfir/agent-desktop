use agent_desktop_core::{AdapterError, ErrorCode, session};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub(super) fn path(session_id: &str) -> Result<PathBuf, AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    Ok(path_for_root(&root, session_id))
}

pub(super) fn lock_path() -> Result<PathBuf, AdapterError> {
    let root = session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))?;
    Ok(root.join(".cursor-overlay-start.lock"))
}

fn path_for_root(root: &Path, session_id: &str) -> PathBuf {
    let name = format!(
        ".cursor-overlay-{:016x}.sock",
        endpoint_hash(root, session_id)
    );
    let path = root.join(&name);
    if path.as_os_str().as_bytes().len() < 100 {
        path
    } else {
        PathBuf::from("/tmp").join(name)
    }
}

fn endpoint_hash(root: &Path, session_id: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in root
        .as_os_str()
        .as_bytes()
        .iter()
        .chain(session_id.as_bytes())
    {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_state_root_uses_a_short_deterministic_socket_path() {
        let root = Path::new("/private/tmp").join("deep".repeat(40));
        let first = path_for_root(&root, "run-1");
        let second = path_for_root(&root, "run-1");

        assert_eq!(first, second);
        assert_eq!(first.parent(), Some(Path::new("/tmp")));
        assert!(first.as_os_str().as_bytes().len() < 100);
    }
}
