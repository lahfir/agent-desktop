use agent_desktop_core::{
    AdapterError, CURSOR_ARRIVAL_TIMEOUT_MS, CursorOverlayControl, ErrorCode,
};
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::child::{MARKER, SOCKET_ENV};

const MAX_INSTRUCTION_BYTES: usize = 4 * 1024;

pub(crate) fn update(control: &CursorOverlayControl) -> Result<(), AdapterError> {
    control.validate()?;
    let socket = super::endpoint::path(control.session_id())?;
    if send(&socket, control).is_ok() {
        return Ok(());
    }
    retire_legacy(control);
    if control.is_disable() || (control.is_transient() && !control.is_travel()) {
        return Ok(());
    }
    let lock_path = super::endpoint::lock_path()?;
    let lock = OpenOptions::new()
        .create(true)
        .read(true)
        .truncate(false)
        .write(true)
        .mode(0o600)
        .open(lock_path)
        .map_err(|error| {
            AdapterError::internal("Could not open the cursor overlay startup lock")
                .with_platform_detail(error.to_string())
        })?;
    lock.lock().map_err(|error| {
        AdapterError::internal("Could not acquire the cursor overlay startup lock")
            .with_platform_detail(error.to_string())
    })?;
    if send(&socket, control).is_ok() {
        return Ok(());
    }
    spawn(&socket, control)
}

fn spawn(socket: &Path, control: &CursorOverlayControl) -> Result<(), AdapterError> {
    let executable = std::env::current_exe().map_err(|error| {
        AdapterError::internal("Could not locate the cursor overlay executable")
            .with_platform_detail(error.to_string())
    })?;
    if executable.file_stem().and_then(|name| name.to_str()) != Some("agent-desktop") {
        return Ok(());
    }
    let payload = serde_json::to_vec(control).map_err(|error| {
        AdapterError::internal("Could not encode macOS cursor overlay control")
            .with_platform_detail(error.to_string())
    })?;
    if payload.len() > MAX_INSTRUCTION_BYTES {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Cursor overlay instruction exceeds the transport limit",
        ));
    }
    let mut child = Command::new(executable)
        .env(MARKER, super::endpoint::PROTOCOL_VERSION)
        .env(SOCKET_ENV, socket)
        .process_group(0)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| {
            AdapterError::internal("Could not start the macOS cursor overlay")
                .with_platform_detail(error.to_string())
        })?;
    let mut stdin = child.stdin.take().ok_or_else(|| {
        AdapterError::internal("Cursor overlay child did not expose its input pipe")
    })?;
    stdin.write_all(&payload).map_err(|error| {
        AdapterError::internal("Could not send the cursor overlay control")
            .with_platform_detail(error.to_string())
    })?;
    drop(stdin);
    let deadline = Instant::now() + Duration::from_millis(CURSOR_ARRIVAL_TIMEOUT_MS);
    while Instant::now() < deadline {
        if UnixStream::connect(socket).is_ok() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(10));
    }
    Err(AdapterError::internal(
        "macOS cursor overlay did not become ready",
    ))
}

fn retire_legacy(control: &CursorOverlayControl) {
    let Ok(socket) = super::endpoint::legacy_path(control.session_id()) else {
        return;
    };
    let disable = CursorOverlayControl::disable(control.session_id().to_owned());
    let _ = send(&socket, &disable);
}

fn send(socket: &Path, control: &CursorOverlayControl) -> Result<(), AdapterError> {
    let mut stream = UnixStream::connect(socket).map_err(|error| {
        AdapterError::internal("Could not connect to the macOS cursor overlay")
            .with_platform_detail(error.to_string())
    })?;
    let payload = serde_json::to_vec(control).map_err(|error| {
        AdapterError::internal("Could not encode macOS cursor overlay control")
            .with_platform_detail(error.to_string())
    })?;
    if payload.len() > MAX_INSTRUCTION_BYTES {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            "Cursor overlay instruction exceeds the transport limit",
        ));
    }
    stream.write_all(&payload).map_err(|error| {
        AdapterError::internal("Could not send the cursor overlay control")
            .with_platform_detail(error.to_string())
    })?;
    let travels = control.is_travel();
    if !travels && !control.is_hide() && !control.is_disable() {
        return Ok(());
    }
    stream
        .shutdown(std::net::Shutdown::Write)
        .map_err(|error| {
            AdapterError::internal("Could not finish the cursor overlay control")
                .with_platform_detail(error.to_string())
        })?;
    let budget = if travels {
        Duration::from_millis(CURSOR_ARRIVAL_TIMEOUT_MS)
    } else {
        Duration::from_secs(4)
    };
    stream.set_read_timeout(Some(budget)).map_err(|error| {
        AdapterError::internal("Could not bound the cursor overlay acknowledgement")
            .with_platform_detail(error.to_string())
    })?;
    let mut acknowledgement = [0_u8; 1];
    match stream.read_exact(&mut acknowledgement) {
        Ok(()) => Ok(()),
        Err(error) if travels => {
            tracing::debug!(%error, "cursor overlay arrival was not acknowledged in time");
            Ok(())
        }
        Err(error) => Err(AdapterError::internal(
            "macOS cursor overlay did not acknowledge the control",
        )
        .with_platform_detail(error.to_string())),
    }
}
