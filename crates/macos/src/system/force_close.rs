use agent_desktop_core::error::{AdapterError, ErrorCode};
use std::time::{Duration, Instant};

pub(crate) fn terminate_app(id: &str, pids: &[i32], timeout: Duration) -> Result<(), AdapterError> {
    signal_pids(id, pids, Signal::Term)?;
    let remaining = remaining_pids_after_wait(pids, timeout);
    if remaining.is_empty() {
        return Ok(());
    }

    signal_pids(id, &remaining, Signal::Kill)?;
    let still_running = remaining_pids_after_wait(&remaining, timeout);
    if still_running.is_empty() {
        return Ok(());
    }

    Err(AdapterError::timeout(format!(
        "App '{id}' still has running pid(s) after force close: {}",
        format_pids(&still_running)
    ))
    .with_suggestion("Retry after checking for save dialogs or helper processes with 'list-apps'."))
}

fn signal_pids(id: &str, pids: &[i32], signal: Signal) -> Result<(), AdapterError> {
    for &pid in pids {
        signal_result(pid, signal).map(|_| ()).map_err(|detail| {
            AdapterError::new(
                ErrorCode::ActionFailed,
                format!("Failed to {} app '{id}' pid {pid}", signal.verb()),
            )
            .with_platform_detail(detail)
            .with_suggestion("Use 'list-apps' to verify the running app before retrying.")
        })?;
    }
    Ok(())
}

fn remaining_pids_after_wait(pids: &[i32], timeout: Duration) -> Vec<i32> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let remaining = running_pids(pids);
        if remaining.is_empty() {
            return remaining;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    running_pids(pids)
}

fn running_pids(pids: &[i32]) -> Vec<i32> {
    pids.iter()
        .copied()
        .filter(|pid| process_is_running(*pid))
        .collect()
}

fn process_is_running(pid: i32) -> bool {
    if let Some(running) = child_process_is_running(pid) {
        return running;
    }
    signal_result(pid, Signal::None).unwrap_or(true)
}

fn child_process_is_running(pid: i32) -> Option<bool> {
    const POSIX_ECHILD: i32 = 10;
    const WNOHANG: i32 = 1;

    unsafe extern "C" {
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
    }
    let mut status = 0;
    match unsafe { waitpid(pid, &mut status, WNOHANG) } {
        child if child == pid => Some(false),
        0 => Some(true),
        _ if std::io::Error::last_os_error().raw_os_error() == Some(POSIX_ECHILD) => None,
        _ => None,
    }
}

fn signal_result(pid: i32, signal: Signal) -> Result<bool, String> {
    const POSIX_ESRCH: i32 = 3;

    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    if unsafe { kill(pid, signal.number()) } == 0 {
        return Ok(true);
    }
    let err = std::io::Error::last_os_error();
    if err.raw_os_error() == Some(POSIX_ESRCH) {
        return Ok(false);
    }
    Err(err.to_string())
}

fn format_pids(pids: &[i32]) -> String {
    pids.iter()
        .map(i32::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Clone, Copy)]
enum Signal {
    None,
    Term,
    Kill,
}

impl Signal {
    fn number(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Term => 15,
            Self::Kill => 9,
        }
    }

    fn verb(self) -> &'static str {
        match self {
            Self::None => "inspect",
            Self::Term => "terminate",
            Self::Kill => "kill",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Child, Command};

    struct ChildGuard(Child);

    impl ChildGuard {
        fn spawn_term_ignoring() -> Self {
            let child = Command::new("/bin/sh")
                .args(["-c", "trap '' TERM; while :; do sleep 1; done"])
                .spawn()
                .unwrap();
            Self(child)
        }

        fn pid(&self) -> i32 {
            self.0.id() as i32
        }

        fn has_exited(&mut self) -> bool {
            !process_is_running(self.pid())
        }
    }

    impl Drop for ChildGuard {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    #[test]
    fn missing_pids_are_not_reported_as_running() {
        assert!(remaining_pids_after_wait(&[999_999], Duration::from_millis(1)).is_empty());
    }

    #[test]
    fn current_pid_remains_running_after_short_wait() {
        let pid = std::process::id() as i32;

        assert_eq!(
            remaining_pids_after_wait(&[999_999, pid], Duration::from_millis(1)),
            vec![pid]
        );
    }

    #[test]
    fn missing_pids_are_accepted_during_signal_race() {
        assert!(signal_pids("missing", &[999_999], Signal::Term).is_ok());
        assert!(signal_pids("missing", &[999_999], Signal::Kill).is_ok());
    }

    #[test]
    fn terminate_app_escalates_to_kill_for_all_remaining_pids() {
        let mut first = ChildGuard::spawn_term_ignoring();
        let mut second = ChildGuard::spawn_term_ignoring();
        let pids = [first.pid(), second.pid()];

        terminate_app("term-ignoring-test", &pids, Duration::from_millis(100)).unwrap();

        assert!(first.has_exited());
        assert!(second.has_exited());
    }
}
