use agent_desktop_core::error::AdapterError;
use std::time::{Duration, Instant};

const KILL_CONFIRM_FLOOR: Duration = Duration::from_millis(500);

pub(crate) fn terminate_app(id: &str, pids: &[i32], timeout: Duration) -> Result<(), AdapterError> {
    let start = Instant::now();
    let mut failures = signal_failures(pids, Signal::Term);
    let remaining = remaining_pids_after_wait(pids, timeout.saturating_sub(start.elapsed()));
    if remaining.is_empty() {
        return Ok(());
    }

    failures.extend(signal_failures(&remaining, Signal::Kill));
    let still_running = remaining_pids_after_wait(&remaining, kill_confirm_budget(timeout, start));
    if still_running.is_empty() {
        return Ok(());
    }

    let mut err = AdapterError::timeout(format!(
        "App '{id}' still has running pid(s) after force close: {}",
        format_pids(&still_running)
    ))
    .with_suggestion("Retry after checking for save dialogs or helper processes with 'list-apps'.");
    if !failures.is_empty() {
        err = err.with_platform_detail(failures.join("; "));
    }
    Err(err)
}

fn kill_confirm_budget(timeout: Duration, start: Instant) -> Duration {
    if timeout.is_zero() {
        return Duration::ZERO;
    }
    timeout
        .saturating_sub(start.elapsed())
        .max(KILL_CONFIRM_FLOOR)
}

fn signal_failures(pids: &[i32], signal: Signal) -> Vec<String> {
    collect_signal_failures(pids, signal, signal_result)
}

fn collect_signal_failures<F>(pids: &[i32], signal: Signal, mut signal_fn: F) -> Vec<String>
where
    F: FnMut(i32, Signal) -> Result<bool, String>,
{
    let mut failures = Vec::new();
    for &pid in pids {
        if let Err(detail) = signal_fn(pid, signal).map(|_| ()) {
            failures.push(format!("pid {pid} {}: {detail}", signal.verb()));
        }
    }
    failures
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
        assert!(signal_failures(&[999_999], Signal::Term).is_empty());
        assert!(signal_failures(&[999_999], Signal::Kill).is_empty());
    }

    #[test]
    fn signal_collection_attempts_every_pid_after_failure() {
        let mut attempted = Vec::new();

        let failures = collect_signal_failures(&[11, 22, 33], Signal::Term, |pid, _signal| {
            attempted.push(pid);
            if pid == 11 {
                Err("operation not permitted".to_owned())
            } else {
                Ok(true)
            }
        });

        assert_eq!(attempted, vec![11, 22, 33]);
        assert_eq!(failures.len(), 1);
        assert!(failures[0].contains("pid 11 terminate"));
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

    #[test]
    fn kill_confirm_budget_keeps_a_small_floor_after_term_timeout() {
        let start = Instant::now() - Duration::from_secs(1);

        assert_eq!(
            kill_confirm_budget(Duration::from_millis(10), start),
            KILL_CONFIRM_FLOOR
        );
    }

    #[test]
    fn kill_confirm_budget_preserves_explicit_zero_timeout() {
        assert_eq!(
            kill_confirm_budget(Duration::ZERO, Instant::now()),
            Duration::ZERO
        );
    }
}
