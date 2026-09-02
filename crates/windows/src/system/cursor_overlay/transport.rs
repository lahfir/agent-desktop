//! Talking to a renderer that may or may not exist yet.
//!
//! One connection carries one control and reads back at most one byte. The
//! read never parks a thread: `PeekNamedPipe` is polled to a deadline and
//! `ReadFile` is called only once bytes are already waiting.
//!
//! That is a departure from the overlapped-`ReadFile`-plus-`CancelIoEx` shape
//! the decision named, and the property the decision was made for is the one
//! kept: no clipboard-shaped park, where a thread sits in a Win32 call past
//! its own deadline with nothing able to reclaim it. Polling reaches that
//! property with no `OVERLAPPED` lifetime to get wrong and no cancellation
//! semantics to get subtly wrong, which for a first implementation is fewer
//! unsafe invariants rather than more.

use std::time::Duration;

use agent_desktop_core::{AdapterError, CursorOverlayControl};

use super::framing;

/// Why a connection attempt did not deliver, which the caller needs in order
/// to decide whether a renderer exists at all.
#[derive(Debug)]
pub(crate) enum ReachOutcome {
    Delivered,
    NoRenderer,
    Unreachable(AdapterError),
}

const POLL_INTERVAL: Duration = Duration::from_millis(4);

#[cfg(target_os = "windows")]
pub(crate) fn reach(name: &str, control: &CursorOverlayControl, budget: Duration) -> ReachOutcome {
    let payload = match framing::encode(control) {
        Ok(payload) => payload,
        Err(error) => return ReachOutcome::Unreachable(error),
    };
    imp::reach(name, &payload, framing::is_acknowledged(control), budget)
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn reach(
    _name: &str,
    _control: &CursorOverlayControl,
    _budget: Duration,
) -> ReachOutcome {
    ReachOutcome::NoRenderer
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{POLL_INTERVAL, ReachOutcome};
    use crate::system::cursor_overlay::framing;
    use agent_desktop_core::{AdapterError, ErrorCode};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY, GetLastError, HANDLE,
        INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile, WriteFile,
    };
    use windows_sys::Win32::System::Pipes::{PeekNamedPipe, WaitNamedPipeW};

    const GENERIC_READ: u32 = 0x8000_0000;
    const GENERIC_WRITE: u32 = 0x4000_0000;

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    struct OwnedHandle(HANDLE);

    impl Drop for OwnedHandle {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    pub(super) fn reach(
        name: &str,
        payload: &[u8],
        expects_acknowledgement: bool,
        budget: Duration,
    ) -> ReachOutcome {
        let deadline = Instant::now() + budget;
        let handle = match connect(name, deadline) {
            Ok(handle) => handle,
            Err(outcome) => return outcome,
        };

        if !crate::system::cursor_overlay::peer::server_is_this_user(handle.0 as isize) {
            return ReachOutcome::Unreachable(AdapterError::internal(
                "The cursor overlay pipe is served by another user's process",
            ));
        }

        let mut written = 0u32;
        let wrote = unsafe {
            WriteFile(
                handle.0,
                payload.as_ptr(),
                payload.len() as u32,
                &mut written,
                std::ptr::null_mut(),
            )
        };
        if wrote == 0 || written as usize != payload.len() {
            return ReachOutcome::Unreachable(win32_error(
                "The cursor overlay control could not be written to its renderer",
            ));
        }

        if !expects_acknowledgement {
            return ReachOutcome::Delivered;
        }
        match await_acknowledgement(&handle, deadline) {
            Ok(()) => ReachOutcome::Delivered,
            Err(error) => ReachOutcome::Unreachable(error),
        }
    }

    fn connect(name: &str, deadline: Instant) -> Result<OwnedHandle, ReachOutcome> {
        let wide_name = wide(name);
        loop {
            let handle = unsafe {
                CreateFileW(
                    wide_name.as_ptr(),
                    GENERIC_READ | GENERIC_WRITE,
                    FILE_SHARE_READ | FILE_SHARE_WRITE,
                    std::ptr::null(),
                    OPEN_EXISTING,
                    0,
                    std::ptr::null_mut(),
                )
            };
            if handle != INVALID_HANDLE_VALUE {
                return Ok(OwnedHandle(handle));
            }
            let code = unsafe { GetLastError() };
            if code == ERROR_FILE_NOT_FOUND {
                return Err(ReachOutcome::NoRenderer);
            }
            if code != ERROR_PIPE_BUSY {
                return Err(ReachOutcome::Unreachable(win32_error(
                    "The cursor overlay renderer could not be reached",
                )));
            }
            if Instant::now() >= deadline {
                return Err(ReachOutcome::Unreachable(
                    AdapterError::new(
                        ErrorCode::Timeout,
                        "The cursor overlay renderer stayed busy for the whole budget",
                    )
                    .with_platform_detail("ERROR_PIPE_BUSY throughout"),
                ));
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            unsafe { WaitNamedPipeW(wide_name.as_ptr(), remaining.as_millis().min(1000) as u32) };
        }
    }

    /// Polls for the byte rather than blocking for it. `ReadFile` is reached
    /// only once `PeekNamedPipe` has already seen data, so no thread waits
    /// inside the OS past this deadline.
    fn await_acknowledgement(handle: &OwnedHandle, deadline: Instant) -> Result<(), AdapterError> {
        loop {
            let mut available = 0u32;
            let peeked = unsafe {
                PeekNamedPipe(
                    handle.0,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if peeked == 0 {
                return Err(win32_error(
                    "The cursor overlay renderer closed before acknowledging",
                ));
            }
            if available > 0 {
                let mut byte = [0u8; 1];
                let mut read = 0u32;
                let ok = unsafe {
                    ReadFile(
                        handle.0,
                        byte.as_mut_ptr(),
                        1,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                };
                if ok == 0 || read != 1 || byte[0] != framing::ACKNOWLEDGEMENT {
                    return Err(win32_error(
                        "The cursor overlay renderer answered something other than an acknowledgement",
                    ));
                }
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(AdapterError::new(
                    ErrorCode::Timeout,
                    "The cursor overlay renderer did not acknowledge within its budget",
                ));
            }
            std::thread::sleep(POLL_INTERVAL);
        }
    }

    fn win32_error(message: &str) -> AdapterError {
        let code = unsafe { GetLastError() };
        AdapterError::internal(message).with_platform_detail(format!("Win32 error {code}"))
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {}

#[cfg(test)]
#[path = "transport_tests.rs"]
mod tests;
