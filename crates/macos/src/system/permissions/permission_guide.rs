use agent_desktop_core::AdapterError;
use std::ffi::{CStr, CString, c_char};
use std::fs::File;
use std::os::fd::AsRawFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const STEPS_ENV: &str = "AGENT_DESKTOP_PERMISSION_GUIDE";
const HOST_ENV: &str = "AGENT_DESKTOP_PERMISSION_GUIDE_HOST";
const LOCK_ENV: &str = "AGENT_DESKTOP_PERMISSION_GUIDE_LOCK";
const LOCK_NAME: &str = "agent-desktop-permission-guide.lock";
const ACCESSIBILITY: u8 = 1;
const SCREEN_RECORDING: u8 = 2;
const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
const HOST_PATH_CAPACITY: usize = 4096;
const COMMAND_HOST_ENVS: [&str; 3] = [
    "AGENT_DESKTOP_INTERNAL_COMMAND_HOST",
    "AGENT_DESKTOP_INTERNAL_HOST_SOCKET",
    "AGENT_DESKTOP_INTERNAL_HOST_CLIENT",
];

pub(crate) fn start() -> Result<&'static str, AdapterError> {
    let steps = missing_steps(
        crate::system::permissions::preflight_accessibility(),
        crate::system::permissions::preflight_screen_recording(),
    );
    if steps == 0 {
        return Ok("not_needed");
    }
    let lock = match single_instance_lock() {
        Ok(Some(lock)) => lock,
        Ok(None) => return Ok("already_running"),
        Err(error) => {
            return Err(
                AdapterError::internal("Could not open the permission guide lock")
                    .with_platform_detail(error.to_string()),
            );
        }
    };
    let executable = std::env::current_exe().map_err(|error| {
        AdapterError::internal("Could not locate the agent-desktop executable")
            .with_platform_detail(error.to_string())
    })?;
    let host = host_app().unwrap_or_else(|| executable.clone());
    let mut command = Command::new(&executable);
    for name in COMMAND_HOST_ENVS {
        command.env_remove(name);
    }
    hand_over(&mut command, &lock);
    let mut child = command
        .env(STEPS_ENV, steps.to_string())
        .env(HOST_ENV, host)
        .process_group(0)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            AdapterError::internal("Could not start the macOS permission guide")
                .with_platform_detail(error.to_string())
        })?;
    std::thread::spawn(move || child.wait());
    Ok("started")
}

pub fn entry_from_env() -> Option<u8> {
    let steps = std::env::var(STEPS_ENV).ok()?;
    Some(if run_child(&steps) { 0 } else { 1 })
}

fn run_child(steps: &str) -> bool {
    let Some(steps) = parse_steps(steps) else {
        return false;
    };
    let Some(host) = std::env::var(HOST_ENV)
        .ok()
        .filter(|path| std::path::Path::new(path).exists())
        .and_then(|path| CString::new(path).ok())
    else {
        return false;
    };
    if !keep_inherited_lock() {
        return false;
    }
    unsafe { agent_desktop_permission_guide_run(steps, host.as_ptr(), granted) == 0 }
}

extern "C" fn granted(step: u8) -> bool {
    let key = match step {
        ACCESSIBILITY => "accessibility",
        SCREEN_RECORDING => "screen_recording",
        _ => return false,
    };
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let mut command = Command::new(executable);
    command.arg("permissions").env_remove(STEPS_ENV);
    crate::system::process::run_with_deadline(
        &mut command,
        "macOS permission guide probe",
        Instant::now() + PROBE_TIMEOUT,
    )
    .is_ok_and(|output| reports_granted(&output.stdout, key))
}

fn reports_granted(stdout: &[u8], key: &str) -> bool {
    serde_json::from_slice::<serde_json::Value>(stdout)
        .is_ok_and(|response| response["data"][key]["state"] == "granted")
}

fn missing_steps(accessibility: bool, screen_recording: bool) -> u8 {
    let mut steps = 0;
    if !accessibility {
        steps |= ACCESSIBILITY;
    }
    if !screen_recording {
        steps |= SCREEN_RECORDING;
    }
    steps
}

fn parse_steps(value: &str) -> Option<u8> {
    value
        .parse::<u8>()
        .ok()
        .filter(|steps| (1..=(ACCESSIBILITY | SCREEN_RECORDING)).contains(steps))
}

fn host_app() -> Option<PathBuf> {
    let mut buffer = [0 as c_char; HOST_PATH_CAPACITY];
    if !unsafe { agent_desktop_permission_guide_host(buffer.as_mut_ptr(), buffer.len()) } {
        return None;
    }
    let path = unsafe { CStr::from_ptr(buffer.as_ptr()) };
    path.to_str().ok().map(PathBuf::from)
}

fn hand_over(command: &mut Command, lock: &File) {
    let fd = lock.as_raw_fd();
    command.env(LOCK_ENV, fd.to_string());
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

fn keep_inherited_lock() -> bool {
    std::env::var(LOCK_ENV)
        .ok()
        .and_then(|fd| fd.parse::<i32>().ok())
        .is_some_and(|fd| unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } == 0)
}

fn single_instance_lock() -> std::io::Result<Option<File>> {
    lock_file(&std::env::temp_dir().join(LOCK_NAME))
}

fn lock_file(path: &Path) -> std::io::Result<Option<File>> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(Some(file));
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(None)
    } else {
        Err(error)
    }
}

unsafe extern "C" {
    fn agent_desktop_permission_guide_host(output: *mut c_char, capacity: usize) -> bool;
    fn agent_desktop_permission_guide_run(
        steps: u8,
        host_path: *const c_char,
        granted: extern "C" fn(u8) -> bool,
    ) -> i32;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guide_covers_only_missing_permissions() {
        assert_eq!(missing_steps(true, true), 0);
        assert_eq!(missing_steps(false, true), ACCESSIBILITY);
        assert_eq!(missing_steps(true, false), SCREEN_RECORDING);
        assert_eq!(
            missing_steps(false, false),
            ACCESSIBILITY | SCREEN_RECORDING
        );
        assert_eq!(parse_steps("3"), Some(3));
        assert_eq!(parse_steps("0"), None);
        assert_eq!(parse_steps("4"), None);
    }

    #[test]
    fn the_guide_lock_stays_held_from_the_check_through_the_child() {
        let path = std::env::temp_dir().join(format!("{LOCK_NAME}.{}.test", std::process::id()));
        let lock = lock_file(&path).unwrap().unwrap();
        let mut command = Command::new("/bin/sleep");
        command.arg("30");
        hand_over(&mut command, &lock);
        let mut child = command.spawn().unwrap();
        drop(lock);
        assert!(lock_file(&path).unwrap().is_none());
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(lock_file(&path).unwrap().is_some());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn grant_probe_reads_only_the_requested_permission() {
        let stdout = br#"{"ok":true,"data":{"accessibility":{"state":"granted"},"screen_recording":{"state":"denied"}}}"#;
        assert!(reports_granted(stdout, "accessibility"));
        assert!(!reports_granted(stdout, "screen_recording"));
        assert!(!reports_granted(b"not json", "accessibility"));
    }
}
