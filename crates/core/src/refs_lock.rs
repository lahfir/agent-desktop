use crate::error::AppError;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const LOCK_TIMEOUT: Duration = Duration::from_secs(2);
const MALFORMED_LOCK_WINDOW: Duration = Duration::from_secs(30);

pub(crate) struct RefStoreLock {
    path: PathBuf,
    token: String,
}

impl RefStoreLock {
    pub(crate) fn acquire(path: &Path) -> Result<Self, AppError> {
        if let Some(dir) = path.parent() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                std::fs::DirBuilder::new()
                    .recursive(true)
                    .mode(0o700)
                    .create(dir)?;
            }
            #[cfg(not(unix))]
            std::fs::create_dir_all(dir)?;
        }
        let start = Instant::now();
        loop {
            let token = lock_token();
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(mut file) => {
                    if let Err(err) =
                        writeln!(file, "{} {} {}", std::process::id(), now_secs(), token)
                    {
                        let _ = std::fs::remove_file(path);
                        return Err(err.into());
                    }
                    return Ok(Self {
                        path: path.to_path_buf(),
                        token,
                    });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    if try_remove_stale_lock(path) {
                        continue;
                    }
                    if start.elapsed() > LOCK_TIMEOUT {
                        return Err(AppError::Internal(format!(
                            "Timed out waiting for ref store lock at {}",
                            path.display()
                        )));
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => return Err(err.into()),
            }
        }
    }
}

fn try_remove_stale_lock(path: &Path) -> bool {
    let snapshot = match LockSnapshot::read(path) {
        Ok(Some(snapshot)) => snapshot,
        Ok(None) => return true,
        Err(()) => return false,
    };
    if !snapshot.is_stale() {
        return false;
    }
    let current = match LockSnapshot::read(path) {
        Ok(Some(current)) => current,
        Ok(None) => return true,
        Err(()) => return false,
    };
    if current.contents != snapshot.contents || current.modified != snapshot.modified {
        return false;
    }
    match std::fs::remove_file(path) {
        Ok(()) => true,
        Err(err) if err.kind() == ErrorKind::NotFound => true,
        Err(_) => false,
    }
}

struct LockSnapshot {
    contents: String,
    modified: Option<SystemTime>,
}

impl LockSnapshot {
    fn read(path: &Path) -> Result<Option<Self>, ()> {
        let contents = match std::fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(()),
        };
        let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok();
        Ok(Some(Self { contents, modified }))
    }

    fn is_stale(&self) -> bool {
        if self.contents.trim().is_empty() {
            return self.modified_age_exceeds(MALFORMED_LOCK_WINDOW);
        }
        match self.pid() {
            Some(pid) => match process_is_alive(pid) {
                Some(true) => self.live_pid_lock_is_stale(pid),
                Some(false) => true,
                None => self.modified_age_exceeds(MALFORMED_LOCK_WINDOW),
            },
            None => self.modified_age_exceeds(MALFORMED_LOCK_WINDOW),
        }
    }

    fn live_pid_lock_is_stale(&self, pid: u32) -> bool {
        if !self.token_shape_matches_pid(pid) {
            return self.modified_age_exceeds(LOCK_TIMEOUT);
        }
        self.modified_age_exceeds(MALFORMED_LOCK_WINDOW)
    }

    fn pid(&self) -> Option<u32> {
        self.contents
            .split_whitespace()
            .next()
            .and_then(|s| s.parse::<u32>().ok())
    }

    fn token(&self) -> Option<&str> {
        self.contents.split_whitespace().nth(2)
    }

    fn owned_by(&self, token: &str) -> bool {
        self.pid() == Some(std::process::id()) && self.token() == Some(token)
    }

    fn token_shape_matches_pid(&self, pid: u32) -> bool {
        self.token()
            .and_then(|token| token.split_once('-'))
            .and_then(|(prefix, _)| prefix.parse::<u32>().ok())
            == Some(pid)
    }

    fn modified_age_exceeds(&self, duration: Duration) -> bool {
        self.modified
            .and_then(|modified| SystemTime::now().duration_since(modified).ok())
            .is_some_and(|age| age > duration)
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn lock_token() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{}-{nanos}", std::process::id())
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> Option<bool> {
    let result = unsafe { libc::kill(pid as i32, 0) };
    if result == 0 {
        return Some(true);
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(errno) if errno == libc::ESRCH => Some(false),
        Some(errno) if errno == libc::EPERM => None,
        _ => None,
    }
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> Option<bool> {
    None
}

impl Drop for RefStoreLock {
    fn drop(&mut self) {
        let should_remove = LockSnapshot::read(&self.path)
            .ok()
            .flatten()
            .is_some_and(|snapshot| snapshot.owned_by(&self.token));
        if should_remove {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn lock_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "agent-desktop-{name}-{}-{}.lock",
            std::process::id(),
            lock_token()
        ))
    }

    #[test]
    fn acquire_removes_lock_on_drop() {
        let path = lock_path("drop");
        {
            let _lock = RefStoreLock::acquire(&path).unwrap();
            assert!(path.exists());
        }
        assert!(!path.exists());
    }

    #[test]
    fn stale_dead_pid_lock_is_replaced() {
        let path = lock_path("stale-pid");
        fs::write(&path, "999999 1 stale").unwrap();
        let _lock = RefStoreLock::acquire(&path).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(contents.starts_with(&std::process::id().to_string()));
    }

    #[test]
    fn old_same_process_lock_with_foreign_token_shape_is_stale() {
        let snapshot = LockSnapshot {
            contents: format!("{} 1 old-token", std::process::id()),
            modified: Some(SystemTime::now() - Duration::from_secs(60)),
        };

        assert!(snapshot.is_stale());
    }

    #[test]
    fn recent_live_pid_lock_is_not_stale() {
        let snapshot = LockSnapshot {
            contents: format!(
                "{} {} {}-token",
                std::process::id(),
                now_secs(),
                std::process::id()
            ),
            modified: Some(SystemTime::now()),
        };

        assert!(!snapshot.is_stale());
    }

    #[test]
    fn old_live_pid_lock_with_foreign_token_shape_is_stale() {
        let snapshot = LockSnapshot {
            contents: "2000 1 1000-old".into(),
            modified: Some(SystemTime::now() - Duration::from_secs(3)),
        };

        assert!(snapshot.live_pid_lock_is_stale(2000));
    }

    #[test]
    fn recent_live_pid_lock_with_matching_token_shape_is_not_stale() {
        let snapshot = LockSnapshot {
            contents: "2000 1 2000-token".into(),
            modified: Some(SystemTime::now() - Duration::from_secs(3)),
        };

        assert!(!snapshot.live_pid_lock_is_stale(2000));
    }

    #[test]
    fn drop_does_not_remove_replaced_lock() {
        let path = lock_path("replaced");
        let lock = RefStoreLock::acquire(&path).unwrap();
        fs::write(
            &path,
            format!("{} {} replacement-token", std::process::id(), now_secs()),
        )
        .unwrap();

        drop(lock);

        assert!(path.exists());
        fs::remove_file(&path).unwrap();
    }
}
