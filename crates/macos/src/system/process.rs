use agent_desktop_core::error::AdapterError;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) fn run_with_timeout(
    command: &mut Command,
    label: &str,
    timeout: Duration,
) -> Result<Output, AdapterError> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| AdapterError::internal(format!("{label}: {e}")))?;

    let stdout_handle = child.stdout.take().map(spawn_drain);
    let stderr_handle = child.stderr.take().map(spawn_drain);
    let started = Instant::now();

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                join_drain(stdout_handle);
                join_drain(stderr_handle);
                return Err(AdapterError::timeout(format!("{label} timed out"))
                    .with_platform_detail(format!("timeout after {timeout:?}")));
            }
            Ok(None) => thread::sleep(Duration::from_millis(20)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                join_drain(stdout_handle);
                join_drain(stderr_handle);
                return Err(AdapterError::internal(format!("{label} status: {e}")));
            }
        }
    };

    let stdout = join_drain(stdout_handle);
    let stderr = join_drain(stderr_handle);
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

fn spawn_drain<R>(mut reader: R) -> thread::JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = reader.read_to_end(&mut buf);
        buf
    })
}

fn join_drain(handle: Option<thread::JoinHandle<Vec<u8>>>) -> Vec<u8> {
    handle.and_then(|h| h.join().ok()).unwrap_or_default()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn run_with_timeout_returns_output_for_successful_process() {
        let mut command = Command::new("/bin/echo");
        command.arg("ok");

        let output = run_with_timeout(&mut command, "echo", Duration::from_secs(1)).unwrap();

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");
    }

    #[test]
    fn run_with_timeout_kills_slow_process() {
        let mut command = Command::new("/bin/sleep");
        command.arg("1");

        let err = run_with_timeout(&mut command, "sleep", Duration::from_millis(10)).unwrap_err();

        assert_eq!(err.code.as_str(), "TIMEOUT");
    }

    #[test]
    fn run_with_timeout_drains_large_stdout_without_deadlock() {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", "yes ABCDEFGHIJ | head -c 200000"]);

        let output = run_with_timeout(&mut command, "yes-head", Duration::from_secs(5)).unwrap();

        assert!(output.status.success());
        assert!(
            output.stdout.len() >= 200_000,
            "expected >=200000 bytes of drained stdout, got {}",
            output.stdout.len()
        );
    }

    #[test]
    fn run_with_timeout_returns_internal_for_missing_binary() {
        let mut command = Command::new("/nonexistent/binary-zzz");

        let err = run_with_timeout(&mut command, "missing", Duration::from_secs(1)).unwrap_err();

        assert_eq!(err.code.as_str(), "INTERNAL");
    }
}
