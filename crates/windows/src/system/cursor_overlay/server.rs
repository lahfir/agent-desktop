//! The renderer's end of the control pipe.
//!
//! `FILE_FLAG_FIRST_PIPE_INSTANCE` is the singleton lock: a second child
//! fails to claim the name and withdraws, which is why claiming happens
//! before any window exists. `PIPE_REJECT_REMOTE_CLIENTS` keeps the channel
//! local, and every accepted connection is checked against this user before
//! its payload is read.

#[cfg(target_os = "windows")]
pub(crate) use imp::{Accepted, ClaimError, Listener};

#[cfg(target_os = "windows")]
mod imp {
    use crate::system::cursor_overlay::{framing, peer};
    use agent_desktop_core::{AdapterError, CursorOverlayControl};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::{
        CloseHandle, ERROR_ACCESS_DENIED, ERROR_PIPE_CONNECTED, GetLastError, HANDLE,
        INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_DUPLEX, ReadFile, WriteFile,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE,
        PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PeekNamedPipe,
    };

    const POLL: Duration = Duration::from_millis(8);

    pub(crate) enum ClaimError {
        /// Another renderer already serves this session. Withdrawing here,
        /// before a window exists, is what keeps a duplicate from drawing.
        AlreadyServed,
        Failed(AdapterError),
    }

    pub(crate) enum Accepted {
        Control(Connection, CursorOverlayControl),
        /// A connection from another user, closed before its payload was
        /// read.
        Refused,
        /// Nothing arrived within the tick.
        Idle,
        Broken(AdapterError),
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    pub(crate) struct Listener {
        handle: HANDLE,
    }

    pub(crate) struct Connection {
        handle: HANDLE,
    }

    impl Connection {
        pub(crate) fn acknowledge(&self) {
            let byte = [framing::ACKNOWLEDGEMENT];
            let mut written = 0u32;
            unsafe {
                WriteFile(
                    self.handle,
                    byte.as_ptr(),
                    1,
                    &mut written,
                    std::ptr::null_mut(),
                );
            }
        }
    }

    impl Listener {
        pub(crate) fn claim(name: &str) -> Result<Self, ClaimError> {
            let wide_name = wide(name);
            let handle = unsafe {
                CreateNamedPipeW(
                    wide_name.as_ptr(),
                    PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
                    PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_REJECT_REMOTE_CLIENTS,
                    1,
                    framing::MAX_CONTROL_BYTES as u32,
                    framing::MAX_CONTROL_BYTES as u32,
                    0,
                    std::ptr::null(),
                )
            };
            if handle == INVALID_HANDLE_VALUE {
                let code = unsafe { GetLastError() };
                if code == ERROR_ACCESS_DENIED {
                    return Err(ClaimError::AlreadyServed);
                }
                return Err(ClaimError::Failed(
                    AdapterError::internal("The cursor overlay pipe could not be created")
                        .with_platform_detail(format!("Win32 error {code}")),
                ));
            }
            Ok(Self { handle })
        }

        /// Waits up to `tick` for a client, reads one control, and hands back
        /// the connection so the caller can decide whether to acknowledge.
        pub(crate) fn accept_next(&self, tick: Duration) -> Accepted {
            let deadline = Instant::now() + tick;
            loop {
                let connected = unsafe { ConnectNamedPipe(self.handle, std::ptr::null_mut()) };
                let code = unsafe { GetLastError() };
                if connected != 0 || code == ERROR_PIPE_CONNECTED {
                    break;
                }
                if Instant::now() >= deadline {
                    return Accepted::Idle;
                }
                std::thread::sleep(POLL);
            }

            if !peer::peer_is_this_user(self.handle as isize) {
                self.disconnect();
                return Accepted::Refused;
            }

            let outcome = match self.read_control(deadline) {
                Ok(Some(control)) => {
                    return Accepted::Control(
                        Connection {
                            handle: self.handle,
                        },
                        control,
                    );
                }
                Ok(None) => Accepted::Idle,
                Err(error) => Accepted::Broken(error),
            };
            self.disconnect();
            outcome
        }

        pub(crate) fn disconnect(&self) {
            unsafe { DisconnectNamedPipe(self.handle) };
        }

        fn read_control(
            &self,
            deadline: Instant,
        ) -> Result<Option<CursorOverlayControl>, AdapterError> {
            loop {
                let mut available = 0u32;
                let peeked = unsafe {
                    PeekNamedPipe(
                        self.handle,
                        std::ptr::null_mut(),
                        0,
                        std::ptr::null_mut(),
                        &mut available,
                        std::ptr::null_mut(),
                    )
                };
                if peeked == 0 {
                    return Ok(None);
                }
                if available > 0 {
                    let mut buffer = vec![0u8; framing::MAX_CONTROL_BYTES];
                    let mut read = 0u32;
                    let ok = unsafe {
                        ReadFile(
                            self.handle,
                            buffer.as_mut_ptr(),
                            available.min(framing::MAX_CONTROL_BYTES as u32),
                            &mut read,
                            std::ptr::null_mut(),
                        )
                    };
                    if ok == 0 || read == 0 {
                        return Ok(None);
                    }
                    buffer.truncate(read as usize);
                    return framing::decode(&buffer).map(Some);
                }
                if Instant::now() >= deadline.max(Instant::now()) && available == 0 {
                    std::thread::sleep(POLL);
                }
                if Instant::now() >= deadline + Duration::from_millis(200) {
                    return Ok(None);
                }
            }
        }
    }

    impl Drop for Listener {
        fn drop(&mut self) {
            unsafe {
                DisconnectNamedPipe(self.handle);
                CloseHandle(self.handle);
            }
        }
    }
}
