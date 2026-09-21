#![cfg(target_os = "macos")]

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn host_identity(directory: &std::path::Path) -> String {
    use std::os::unix::fs::MetadataExt;
    let executable = std::path::Path::new(env!("CARGO_BIN_EXE_agent-desktop"));
    let metadata = std::fs::metadata(executable).unwrap();
    serde_json::json!({
        "protocol": 1,
        "root": directory,
        "session": null,
        "executable": executable,
        "revision": [metadata.dev(), metadata.ino(), metadata.size()],
        "modified": [metadata.mtime(), metadata.mtime_nsec()],
        "helper": std::env::var_os("AGENT_DESKTOP_MACOS_HELPER_PATH"),
    })
    .to_string()
}

fn future_deadline() -> u64 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) },
        0
    );
    time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64 + 60_000_000_000
}

struct HostProcess(std::process::Child);

impl Drop for HostProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn socket_host_survives_a_bad_client_and_serves_separate_invocations() {
    use std::os::unix::fs::DirBuilderExt;
    let directory = std::path::Path::new("/tmp").join(format!(
        "ad-host-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let socket = directory.join("h.sock");
    let mut child = HostProcess(
        Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
            .env("AGENT_DESKTOP_INTERNAL_COMMAND_HOST", "1")
            .env("AGENT_DESKTOP_INTERNAL_HOST_SOCKET", &socket)
            .env_remove("AGENT_DESKTOP_SESSION")
            .env("AGENT_DESKTOP_HOME", &directory)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if UnixStream::connect(&socket).is_ok() {
            break;
        }
        if child.0.try_wait().unwrap().is_some() {
            let mut diagnostic = String::new();
            child
                .0
                .stdout
                .take()
                .unwrap()
                .read_to_string(&mut diagnostic)
                .unwrap();
            child
                .0
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut diagnostic)
                .unwrap();
            panic!("host exited before binding: {diagnostic}");
        }
        assert!(Instant::now() < deadline, "host did not bind");
        std::thread::sleep(Duration::from_millis(10));
    }
    for (request, expected) in [("[\n", false), ("[\"version\"]\n[\"version\"]\n", false)] {
        let mut client = UnixStream::connect(&socket).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        client.write_all(request.as_bytes()).unwrap();
        let mut reply = Vec::new();
        client.read_to_end(&mut reply).unwrap();
        let envelope: serde_json::Value = serde_json::from_slice(&reply).unwrap();
        assert_eq!(envelope["ok"], expected);
        assert_eq!(reply.iter().filter(|byte| **byte == b'\n').count(), 1);
    }
    let identity = host_identity(&directory);
    let mut expired = UnixStream::connect(&socket).unwrap();
    expired
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    writeln!(
        expired,
        "{}",
        serde_json::json!({
            "identity": identity,
            "args": ["session", "start"],
            "directory": directory,
            "agent_id": null,
            "expires_ns": 0,
        })
    )
    .unwrap();
    let mut rejected = String::new();
    expired.read_to_string(&mut rejected).unwrap();
    let rejected: serde_json::Value = serde_json::from_str(&rejected).unwrap();
    assert_eq!(rejected["error"]["code"], "TIMEOUT");
    assert_eq!(
        rejected["error"]["disposition"]["delivery"],
        "not_delivered"
    );
    assert!(
        !directory.join("sessions").exists(),
        "expired request must not create a session"
    );
    for _ in 0..2 {
        let mut client = UnixStream::connect(&socket).unwrap();
        client
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let request = serde_json::json!({
            "identity": identity,
            "args": ["version"],
            "directory": directory,
            "agent_id": null,
            "expires_ns": future_deadline(),
        });
        writeln!(client, "{request}").unwrap();
        let mut reply = String::new();
        client.read_to_string(&mut reply).unwrap();
        let reply: serde_json::Value = serde_json::from_str(&reply).unwrap();
        assert_eq!(reply["ok"], true, "{reply}");
    }
    assert!(child.0.try_wait().unwrap().is_none());
}

#[test]
fn experimental_cli_starts_and_reuses_its_host() {
    use std::hash::{Hash, Hasher};
    let directory = std::env::temp_dir().join(format!("ad-host-client-{}", std::process::id()));
    let mut hash = std::collections::hash_map::DefaultHasher::new();
    host_identity(&directory).hash(&mut hash);
    let socket = std::env::temp_dir().join(format!(
        "agent-desktop-host-{}/{:016x}.sock",
        unsafe { libc::geteuid() },
        hash.finish(),
    ));
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
            .arg("version")
            .env("AGENT_DESKTOP_INTERNAL_HOST_CLIENT", "1")
            .env_remove("AGENT_DESKTOP_INTERNAL_COMMAND_HOST")
            .env_remove("AGENT_DESKTOP_SESSION")
            .env("AGENT_DESKTOP_HOME", &directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    };
    let first = invoke();
    let concurrent = invoke();
    let first = first.wait_with_output().unwrap();
    let concurrent = concurrent.wait_with_output().unwrap();
    let stream = UnixStream::connect(&socket).unwrap();
    let mut pid: libc::pid_t = 0;
    let mut size = std::mem::size_of_val(&pid) as libc::socklen_t;
    assert_eq!(
        unsafe {
            libc::getsockopt(
                stream.as_raw_fd(),
                libc::SOL_LOCAL,
                libc::LOCAL_PEERPID,
                (&mut pid as *mut libc::pid_t).cast(),
                &mut size,
            )
        },
        0
    );
    drop(stream);
    let second = invoke().wait_with_output().unwrap();
    let reused = UnixStream::connect(&socket).unwrap();
    let mut second_pid: libc::pid_t = 0;
    let lookup = unsafe {
        libc::getsockopt(
            reused.as_raw_fd(),
            libc::SOL_LOCAL,
            libc::LOCAL_PEERPID,
            (&mut second_pid as *mut libc::pid_t).cast(),
            &mut size,
        )
    };
    drop(reused);
    assert!(pid > 0);
    unsafe { libc::kill(pid, libc::SIGTERM) };
    assert_eq!(lookup, 0);
    assert_eq!(
        second_pid, pid,
        "separate invocations must reuse the native owner"
    );
    for result in [first, concurrent, second] {
        let reply: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert!(result.status.success(), "{reply}");
        assert_eq!(reply["ok"], true);
        assert_eq!(reply["command"], "version");
    }
}

#[test]
fn unread_reply_exits_without_shutdown_flush_or_next_dispatch() {
    let (sender, mut receiver) = UnixStream::pair().unwrap();
    let size: libc::c_int = 4096;
    assert_eq!(
        unsafe {
            libc::setsockopt(
                sender.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_SNDBUF,
                (&size as *const libc::c_int).cast(),
                std::mem::size_of_val(&size) as libc::socklen_t,
            )
        },
        0
    );
    let output: OwnedFd = sender.into();
    let mut child = Command::new(env!("CARGO_BIN_EXE_agent-desktop"))
        .env("AGENT_DESKTOP_INTERNAL_COMMAND_HOST", "1")
        .env_remove("AGENT_DESKTOP_SESSION")
        .env(
            "AGENT_DESKTOP_HOME",
            std::env::temp_dir().join(format!("ad-host-output-{}", std::process::id())),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::from(output))
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"[\"skills\",\"get\",\"desktop\",\"--full\"]\n[\"version\"]\n")
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("host failed to exit while its output remained unread");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut diagnostics = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut diagnostics)
        .unwrap();
    assert_eq!(status.code(), Some(1), "{diagnostics}");
    assert!(diagnostics.contains("timed out"), "{diagnostics}");
    let mut bytes = Vec::new();
    receiver.read_to_end(&mut bytes).unwrap();
    assert!(bytes.starts_with(b"{\"version\":"));
    assert!(
        !bytes.contains(&b'\n'),
        "must not finish or dispatch the next reply"
    );
}
