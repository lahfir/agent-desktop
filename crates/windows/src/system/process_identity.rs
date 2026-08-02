use agent_desktop_core::{AdapterError, ProcessId};

/// The process-generation token KTD3 defines: a creation-time-derived identity
/// that survives HWND recycling, mirroring macOS's
/// `"macos-proc-v1:{start_seconds}:{start_microseconds}"` shape.
///
/// Windows FILETIME is 100-nanosecond ticks since 1601; the token keeps the
/// integer second and the sub-second tick so two processes started in the same
/// second stay distinct, the same way the macOS token's microsecond field
/// does. A token is read from the process handle KTD3 already needs, so the
/// identity never costs a separate enumeration.
const TOKEN_PREFIX: &str = "windows-proc-v1";
const TICKS_PER_SECOND: u64 = 10_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProcessIdentity {
    pid: ProcessId,
    creation_seconds: u64,
    creation_subtick: u64,
}

impl ProcessIdentity {
    /// Captures the process-generation identity for `pid`.
    ///
    /// `None` is the honest answer for a process whose token cannot be read
    /// (an elevated-process handle the caller cannot open, per the split-
    /// integrity measurement A16-12): the window still lists, with
    /// `process_instance: None`, and fails closed on resolution (KTD3).
    #[cfg(target_os = "windows")]
    pub(crate) fn capture(pid: ProcessId) -> Result<Option<Self>, AdapterError> {
        use windows_sys::Win32::Foundation::{CloseHandle, FILETIME};
        use windows_sys::Win32::System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
        };

        let raw_pid = u32::from(pid);
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, raw_pid) };
        if process.is_null() {
            return Ok(None);
        }
        let mut created = FILETIME::default();
        let mut exit = FILETIME::default();
        let mut kernel = FILETIME::default();
        let mut user = FILETIME::default();
        let read_ok =
            unsafe { GetProcessTimes(process, &mut created, &mut exit, &mut kernel, &mut user) };
        unsafe { CloseHandle(process) };
        if read_ok == 0 {
            return Ok(None);
        }
        let ticks = (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime);
        if ticks == 0 {
            return Ok(None);
        }
        Ok(Some(Self {
            pid,
            creation_seconds: ticks / TICKS_PER_SECOND,
            creation_subtick: ticks % TICKS_PER_SECOND,
        }))
    }

    #[cfg(not(target_os = "windows"))]
    pub(crate) fn capture(_pid: ProcessId) -> Result<Option<Self>, AdapterError> {
        Ok(None)
    }

    pub(crate) fn token(self) -> String {
        format!(
            "{TOKEN_PREFIX}:{}:{}",
            self.creation_seconds, self.creation_subtick
        )
    }

    /// Whether this identity still matches the process at `pid` right now.
    pub(crate) fn still_matches(self) -> Result<bool, AdapterError> {
        Ok(Self::capture(self.pid)?.is_some_and(|current| current == self))
    }
}

pub(crate) fn token_for_pid(pid: ProcessId) -> Result<Option<String>, AdapterError> {
    Ok(ProcessIdentity::capture(pid)?.map(ProcessIdentity::token))
}

/// Verifies a stored token against the process's current generation.
///
/// A recycled PID whose process is a different generation fails closed; a PID
/// whose process is gone altogether reads `None` and also fails. This is the
/// check a recycled HWND on a different process generation trips.
pub(crate) fn matches_instance(pid: ProcessId, token: &str) -> Result<bool, AdapterError> {
    let expected = match parse_token(pid, token)? {
        Some(identity) => identity,
        None => return Ok(false),
    };
    expected.still_matches()
}

/// Parses a token back into a comparable identity, or `None` for an
/// unrecognised shape - which must not match anything.
fn parse_token(pid: ProcessId, token: &str) -> Result<Option<ProcessIdentity>, AdapterError> {
    let mut parts = token.split(':');
    if parts.next() != Some(TOKEN_PREFIX) {
        return Ok(None);
    }
    let (seconds, subtick) = match (parts.next(), parts.next()) {
        (Some(seconds), Some(subtick)) => match (seconds.parse::<u64>(), subtick.parse::<u64>()) {
            (Ok(seconds), Ok(subtick)) => (seconds, subtick),
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    Ok(Some(ProcessIdentity {
        pid,
        creation_seconds: seconds,
        creation_subtick: subtick,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_shape_mirrors_macos_is_two_component_creation_time() {
        let identity = ProcessIdentity {
            pid: ProcessId::new(1),
            creation_seconds: 1_700_000_000,
            creation_subtick: 123_456,
        };

        assert_eq!(identity.token(), "windows-proc-v1:1700000000:123456");
    }

    #[test]
    fn two_processes_started_in_the_same_second_stay_distinct() {
        let first = ProcessIdentity {
            pid: ProcessId::new(1),
            creation_seconds: 1_700_000_000,
            creation_subtick: 100,
        };
        let second = ProcessIdentity {
            pid: ProcessId::new(1),
            creation_seconds: 1_700_000_000,
            creation_subtick: 200,
        };

        assert_ne!(first, second);
        assert_ne!(first.token(), second.token());
    }

    #[test]
    fn a_different_generation_is_a_different_identity() {
        let before = ProcessIdentity {
            pid: ProcessId::new(1),
            creation_seconds: 1_700_000_000,
            creation_subtick: 100,
        };
        let after = ProcessIdentity {
            pid: ProcessId::new(1),
            creation_seconds: 1_700_000_100,
            creation_subtick: 100,
        };

        assert_ne!(before, after);
        assert_eq!(before.pid, after.pid);
        assert_ne!(before.token(), after.token());
    }

    #[test]
    fn a_shape_mismatch_matches_nothing() {
        let pid = ProcessId::new(1);

        assert!(!matches_instance(pid, "macos-proc-v1:1:2").unwrap());
        assert!(!matches_instance(pid, "windows-proc-v2:1:2").unwrap());
        assert!(!matches_instance(pid, "windows-proc-v1:nope:2").unwrap());
        assert!(!matches_instance(pid, "").unwrap());
    }

    #[test]
    fn a_fresh_token_matches_its_own_process() {
        let pid = ProcessId::from(std::process::id());
        let Some(token) = token_for_pid(pid).unwrap() else {
            return;
        };

        assert!(
            matches_instance(pid, &token).unwrap(),
            "a freshly captured token must match the same process"
        );
    }
}
