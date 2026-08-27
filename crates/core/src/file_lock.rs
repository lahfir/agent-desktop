use std::fs::File;
use std::path::Path;
use std::time::Duration;

use crate::{AdapterError, Deadline, ErrorCode};

/// An advisory lock released by closing its descriptor, deliberately without
/// an explicit unlock on drop.
///
/// The two are identical for a lease nobody shared and opposite for one handed
/// to a child. `duplicate_inheritable` hands out an `F_DUPFD` descriptor, which
/// shares this lock's *open file description*, and an advisory lock lives on
/// that description rather than on any one descriptor. Unlocking therefore
/// released it for every holder of the description, so a parent dropping its
/// lease after a child had adopted the inherited descriptor revoked the
/// child's exclusivity — the single guarantee the lease exists to provide —
/// and let a second acquirer through. Whether that happened depended on which
/// of the two calls landed first, which is why it surfaced as an intermittent
/// failure of the adoption test rather than a constant one. Closing releases
/// only when the last descriptor closes, which is what the adopt path is
/// written against.
pub(crate) struct FileLock {
    /// Held for its lifetime rather than read: closing it is what releases the
    /// lock, so the value being alive *is* its purpose. Unix additionally reads
    /// it to duplicate the descriptor for a child.
    #[cfg_attr(not(unix), allow(dead_code))]
    file: File,
    #[cfg(unix)]
    contention_count: u64,
}

impl FileLock {
    pub(crate) fn acquire(
        path: &Path,
        deadline: Deadline,
        purpose: &str,
    ) -> Result<Self, AdapterError> {
        let file = crate::private_file::open_private_lock(path, true).map_err(io_error)?;
        lock_file(file, deadline, purpose, path)
    }

    pub(crate) fn is_held(path: &Path) -> bool {
        let Ok(file) = crate::private_file::open_private_lock(path, false) else {
            return false;
        };
        match file.try_lock() {
            Ok(()) => {
                let _ = file.unlock();
                false
            }
            Err(std::fs::TryLockError::WouldBlock) => true,
            Err(std::fs::TryLockError::Error(_)) => true,
        }
    }

    #[cfg(unix)]
    pub(crate) fn contention_count(&self) -> u64 {
        self.contention_count
    }

    #[cfg(unix)]
    pub(crate) fn duplicate_inheritable(&self) -> Result<std::os::fd::OwnedFd, AdapterError> {
        use std::os::fd::{AsRawFd, FromRawFd};

        let duplicated = unsafe { libc::fcntl(self.file.as_raw_fd(), libc::F_DUPFD, 3) };
        if duplicated < 0 {
            return Err(io_error(std::io::Error::last_os_error()));
        }
        Ok(unsafe { std::os::fd::OwnedFd::from_raw_fd(duplicated) })
    }

    #[cfg(unix)]
    pub(crate) fn adopt_inherited(
        raw_fd: std::os::fd::RawFd,
        canonical_path: &Path,
        deadline: Deadline,
        purpose: &str,
    ) -> Result<Self, AdapterError> {
        use std::os::fd::{FromRawFd, OwnedFd};
        use std::os::unix::fs::MetadataExt;

        let duplicated = unsafe { libc::fcntl(raw_fd, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicated < 0 {
            return Err(io_error(std::io::Error::last_os_error()));
        }
        let original_flags = unsafe { libc::fcntl(raw_fd, libc::F_GETFD) };
        if original_flags < 0
            || unsafe { libc::fcntl(raw_fd, libc::F_SETFD, original_flags | libc::FD_CLOEXEC) } != 0
        {
            unsafe { libc::close(duplicated) };
            return Err(io_error(std::io::Error::last_os_error()));
        }
        let inherited = File::from(unsafe { OwnedFd::from_raw_fd(duplicated) });
        let inherited_meta =
            crate::private_file::validate_private_regular(&inherited).map_err(io_error)?;
        let canonical =
            crate::private_file::open_private_lock(canonical_path, false).map_err(io_error)?;
        let canonical_meta = canonical.metadata().map_err(io_error)?;
        if inherited_meta.dev() != canonical_meta.dev()
            || inherited_meta.ino() != canonical_meta.ino()
        {
            return Err(AdapterError::new(
                ErrorCode::PolicyDenied,
                "Inherited interaction lease does not identify the canonical lock file",
            ));
        }
        lock_file(inherited, deadline, purpose, canonical_path)
    }
}

/// Abandons a lock it decided not to return by dropping the file, never by
/// unlocking it.
///
/// The distinction matters on the adopt path, where `file` is a dup of an
/// inherited descriptor and therefore shares its open file description with
/// whoever handed it over. `try_lock` on a description that already holds the
/// lock succeeds immediately, so the expiry branch below is reachable with a
/// lock this call did not take — and unlocking there would release it for the
/// parent too, handing back `TIMEOUT` while quietly stripping the exclusivity
/// the parent still believes it holds. Dropping releases a description this
/// call opened and leaves a shared one alone, which is right in both cases.
fn lock_file(
    file: File,
    deadline: Deadline,
    purpose: &str,
    path: &Path,
) -> Result<FileLock, AdapterError> {
    let mut contention_count = 0_u64;
    loop {
        if deadline.is_expired() {
            return Err(lock_timeout(deadline, purpose, path, contention_count));
        }
        match file.try_lock() {
            Ok(()) => {
                if deadline.is_expired() {
                    return Err(lock_timeout(deadline, purpose, path, contention_count));
                }
                return Ok(FileLock {
                    file,
                    #[cfg(unix)]
                    contention_count,
                });
            }
            Err(std::fs::TryLockError::WouldBlock) => {
                contention_count = contention_count.saturating_add(1);
                let remaining = deadline.remaining();
                if remaining.is_zero() {
                    return Err(lock_timeout(deadline, purpose, path, contention_count));
                }
                std::thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(std::fs::TryLockError::Error(error)) => {
                return Err(AdapterError::new(
                    ErrorCode::Internal,
                    format!("Failed to acquire {purpose}: {error}"),
                ));
            }
        }
    }
}

fn lock_timeout(
    deadline: Deadline,
    purpose: &str,
    path: &Path,
    contention_count: u64,
) -> AdapterError {
    deadline.timeout_error().with_details(serde_json::json!({
        "kind": "lock_timeout",
        "purpose": purpose,
        "path": path.display().to_string(),
        "contention_count": contention_count,
    }))
}

fn io_error(error: std::io::Error) -> AdapterError {
    AdapterError::new(ErrorCode::Internal, error.to_string())
}
