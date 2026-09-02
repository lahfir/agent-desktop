//! Reaching the renderer for a session, and starting one when there is none.
//!
//! Connect first, classify a failure before acting on it, and spawn only for
//! a control that may bring a renderer into existence. A `Disable` that
//! spawned would start a renderer in order to tell it to stop, and dispatch
//! sends `Hide`/`Show` around every mutating command in a headed session —
//! which would fork a detached renderer per command.

use std::time::Duration;

use agent_desktop_core::{AdapterError, CursorOverlayControl, ErrorCode};

use super::framing;
use super::pipe_name;
use super::transport::{self, ReachOutcome};

/// Matches the arrival ceiling core already imposes on a travel, and is what
/// a fresh renderer has to get its window up inside.
const ENABLE_BUDGET: Duration =
    Duration::from_millis(agent_desktop_core::CURSOR_ARRIVAL_TIMEOUT_MS);

/// Teardown is allowed longer, because a caller of `disable` is waiting for
/// the window to be gone rather than for a frame to land.
const TEARDOWN_BUDGET: Duration = Duration::from_secs(4);

pub(crate) fn update(control: &CursorOverlayControl) -> Result<(), AdapterError> {
    control.validate()?;
    let root = state_root()?;
    let name = pipe_name::pipe_name(&root, control.session_id());
    let budget = budget_for(control);

    match transport::reach(&name, control, budget) {
        ReachOutcome::Delivered => return Ok(()),
        ReachOutcome::Unreachable(error) => return Err(error),
        ReachOutcome::NoRenderer => {}
    }

    if !framing::may_spawn(control) {
        return Ok(());
    }

    start_renderer(&name, control, budget)
}

fn budget_for(control: &CursorOverlayControl) -> Duration {
    if control.is_disable() {
        TEARDOWN_BUDGET
    } else {
        ENABLE_BUDGET
    }
}

fn state_root() -> Result<std::path::PathBuf, AdapterError> {
    agent_desktop_core::session::agent_desktop_dir()
        .map_err(|error| AdapterError::new(ErrorCode::InvalidArgs, error.to_string()))
}

#[cfg(target_os = "windows")]
fn start_renderer(
    name: &str,
    control: &CursorOverlayControl,
    budget: Duration,
) -> Result<(), AdapterError> {
    imp::start_renderer(name, control, budget)
}

#[cfg(not(target_os = "windows"))]
fn start_renderer(
    _name: &str,
    _control: &CursorOverlayControl,
    _budget: Duration,
) -> Result<(), AdapterError> {
    Err(AdapterError::not_supported("update_cursor_overlay"))
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{ENABLE_BUDGET, pipe_name, transport};
    use agent_desktop_core::{AdapterError, CursorOverlayControl, ErrorCode};
    use std::time::{Duration, Instant};
    use transport::ReachOutcome;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
    use windows_sys::Win32::System::Threading::{
        CREATE_NO_WINDOW, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DETACHED_PROCESS,
        PROCESS_INFORMATION, STARTUPINFOW,
    };

    /// Spawns only from the CLI's own image. Under an FFI host
    /// `current_exe()` is the host process, which must never be re-launched
    /// as a renderer, and a test binary must not fork one either.
    ///
    /// The refusal is an `Err` rather than macOS's `Ok(())`, because
    /// dispatch turns `Ok` into `rendered: true` - which would claim an
    /// overlay that was never started.
    fn executable() -> Result<std::path::PathBuf, AdapterError> {
        let path = std::env::current_exe().map_err(|error| {
            AdapterError::internal("The running executable could not be resolved")
                .with_platform_detail(error.to_string())
        })?;
        let stem = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or_default();
        if stem != "agent-desktop" {
            return Err(AdapterError::not_supported("update_cursor_overlay")
                .with_platform_detail(format!(
                    "the overlay renderer is only started from the agent-desktop image, not '{stem}'"
                )));
        }
        Ok(path)
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Starts the renderer with **no inherited handles at all**.
    ///
    /// This is why the spawn is a raw `CreateProcessW` rather than
    /// `std::process::Command`: `Command` passes `bInheritHandles = TRUE`
    /// whenever it configures stdio, and does not restrict what is
    /// inherited. A detached renderer that inherits its caller's stdout
    /// keeps that pipe open for its whole life, so any shell reading the
    /// command's output blocks until the overlay is torn down - measured,
    /// and the reason this path looks like `launch.rs` rather than like the
    /// macOS spawn it otherwise mirrors.
    ///
    /// Losing the inherited stdin costs nothing: the control that started
    /// the child is delivered again over the pipe by `await_renderer`, which
    /// has to connect anyway to read the acknowledgement.
    pub(super) fn start_renderer(
        name: &str,
        control: &CursorOverlayControl,
        budget: Duration,
    ) -> Result<(), AdapterError> {
        let image = executable()?;
        let mut command_line = wide(&format!(
            "\"{}\" {}",
            image.display(),
            pipe_name::child_arguments(control.session_id()).join(" ")
        ));
        let mut environment = child_environment();

        let mut startup: STARTUPINFOW = unsafe { std::mem::zeroed() };
        startup.cb = std::mem::size_of::<STARTUPINFOW>() as u32;
        let mut information: PROCESS_INFORMATION = unsafe { std::mem::zeroed() };
        let started = unsafe {
            CreateProcessW(
                std::ptr::null(),
                command_line.as_mut_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                DETACHED_PROCESS | CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT,
                environment.as_mut_ptr().cast(),
                std::ptr::null(),
                &startup,
                &mut information,
            )
        };
        if started == 0 {
            let code = unsafe { GetLastError() };
            return Err(
                AdapterError::internal("The cursor overlay renderer could not be started")
                    .with_platform_detail(format!("Win32 error {code}")),
            );
        }
        unsafe {
            CloseHandle(information.hThread);
            CloseHandle(information.hProcess);
        }

        await_renderer(name, control, budget)
    }

    /// This process's environment plus the marker that turns the child into
    /// a renderer, as the double-null-terminated block `CreateProcessW`
    /// wants.
    fn child_environment() -> Vec<u16> {
        let mut block: Vec<u16> = Vec::new();
        for (key, value) in std::env::vars() {
            if key.eq_ignore_ascii_case(pipe_name::CHILD_MARKER) {
                continue;
            }
            block.extend(format!("{key}={value}").encode_utf16());
            block.push(0);
        }
        block.extend(
            format!(
                "{}={}",
                pipe_name::CHILD_MARKER,
                pipe_name::PROTOCOL_GENERATION
            )
            .encode_utf16(),
        );
        block.push(0);
        block.push(0);
        block
    }

    /// The control that started the child is delivered here, over the pipe,
    /// once the child has claimed its name and shown its window. Nothing is
    /// carried on stdin, so nothing of the caller's is inherited.
    fn await_renderer(
        name: &str,
        control: &CursorOverlayControl,
        budget: Duration,
    ) -> Result<(), AdapterError> {
        let deadline = Instant::now() + budget;
        loop {
            match transport::reach(name, control, ENABLE_BUDGET) {
                ReachOutcome::Delivered => return Ok(()),
                ReachOutcome::Unreachable(error) => return Err(error),
                ReachOutcome::NoRenderer => {}
            }
            if Instant::now() >= deadline {
                return Err(AdapterError::new(
                    ErrorCode::Timeout,
                    "The cursor overlay renderer did not come up within its budget",
                ));
            }
            std::thread::sleep(Duration::from_millis(8));
        }
    }
}
#[cfg(test)]
#[path = "spawn_tests.rs"]
mod tests;
