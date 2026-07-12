use std::path::Path;

use crate::{AppError, Deadline, file_lock::FileLock};

const LOCK_TIMEOUT_MS: u64 = 2_000;

pub(crate) struct RefStoreLock {
    _lock: FileLock,
}

impl RefStoreLock {
    pub(crate) fn acquire(path: &Path) -> Result<Self, AppError> {
        let deadline = Deadline::after(LOCK_TIMEOUT_MS)?;
        Self::acquire_with_deadline(path, deadline)
    }

    pub(crate) fn acquire_with_deadline(path: &Path, deadline: Deadline) -> Result<Self, AppError> {
        Ok(Self {
            _lock: FileLock::acquire(path, deadline, "ref store lock")?,
        })
    }
}

pub(crate) fn lock_holder_is_live(lock_path: &Path) -> bool {
    FileLock::is_held(lock_path)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn lock_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "agent-desktop-{name}-{}-{}.lock",
            std::process::id(),
            crate::refs::new_snapshot_id()
        ))
    }

    #[test]
    fn lock_is_released_without_deleting_the_lock_file() {
        let path = lock_path("release");
        {
            let _lock = RefStoreLock::acquire(&path).unwrap();
            assert!(lock_holder_is_live(&path));
        }
        assert!(path.is_file());
        assert!(!lock_holder_is_live(&path));
        let _lock = RefStoreLock::acquire(&path).unwrap();
    }

    #[test]
    fn contention_obeys_the_callers_deadline() {
        let path = lock_path("deadline");
        let _held = RefStoreLock::acquire(&path).unwrap();
        let deadline = Deadline::after(10).unwrap();
        let error = match RefStoreLock::acquire_with_deadline(&path, deadline) {
            Ok(_) => panic!("contended lock unexpectedly acquired"),
            Err(error) => error,
        };
        assert_eq!(error.code(), "TIMEOUT");
        assert!(deadline.elapsed() < Duration::from_secs(1));
    }
}
