use std::cell::Cell;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;
use std::os::unix::net::UnixStream;
use std::rc::Rc;

use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use reis::PendingRequestResult;
use reis::ei;
use reis::event::{DeviceCapability, EiEvent, EiEventConverter};
use reis::handshake::EiHandshaker;
use rustix::event::{PollFd, PollFlags, Timespec, poll};

const CLIENT_NAME: &str = "agent-desktop";
const CALLBACK_VERSION: u32 = 1;

/// One libei sender connection to KWin, from the handshake to a resumed
/// virtual keyboard. Every wait is bounded by the caller's deadline.
pub(crate) struct EisLink {
    context: ei::Context,
    converter: EiEventConverter,
    keyboard: Option<reis::event::Device>,
    resumed: bool,
    sequence: u32,
}

impl EisLink {
    pub(crate) fn connect(stream: UnixStream, deadline: Deadline) -> Result<Self, AdapterError> {
        let context = ei::Context::new(stream).map_err(|error| failure("connect", &error))?;
        let mut handshaker = EiHandshaker::new(CLIENT_NAME, ei::handshake::ContextType::Sender);
        let response = loop {
            read(&context, deadline)?;
            let mut response = None;
            while let Some(result) = context.pending_event() {
                let PendingRequestResult::Request(event) = result else {
                    continue;
                };
                response = handshaker
                    .handle_event(event)
                    .map_err(|error| failure("handshake", &error))?;
                if response.is_some() {
                    break;
                }
            }
            if let Some(response) = response {
                break response;
            }
        };
        let converter = EiEventConverter::new(&context, response);
        Ok(Self {
            context,
            converter,
            keyboard: None,
            resumed: false,
            sequence: 0,
        })
    }

    /// Binds the seat's keyboard and waits until KWin resumes it, returning
    /// the XKB keymap it carries.
    pub(crate) fn await_keyboard(&mut self, deadline: Deadline) -> Result<Vec<u8>, AdapterError> {
        while self.keyboard.is_none() || !self.resumed {
            match self.next_event(deadline)? {
                EiEvent::SeatAdded(added) => {
                    added
                        .seat
                        .bind_capabilities(DeviceCapability::Keyboard.into());
                    self.flush()?;
                }
                EiEvent::DeviceAdded(added)
                    if added.device.has_capability(DeviceCapability::Keyboard) =>
                {
                    self.keyboard = Some(added.device);
                }
                EiEvent::DeviceResumed(resumed)
                    if self.keyboard.as_ref() == Some(&resumed.device) =>
                {
                    self.resumed = true;
                }
                EiEvent::DevicePaused(paused) if self.keyboard.as_ref() == Some(&paused.device) => {
                    self.resumed = false;
                }
                EiEvent::Disconnected(disconnected) => {
                    return Err(refused(&format!("{:?}", disconnected.reason)));
                }
                _ => {}
            }
        }
        let keymap = self
            .keyboard
            .as_ref()
            .and_then(reis::event::Device::keymap)
            .ok_or_else(|| refused("the KWin keyboard carries no keymap"))?;
        let mut bytes = vec![0; usize::try_from(keymap.size).unwrap_or(0)];
        std::fs::File::from(
            keymap
                .fd
                .try_clone()
                .map_err(|error| failure("keymap", &error))?,
        )
        .read_exact_at(&mut bytes, 0)
        .map_err(|error| failure("keymap", &error))?;
        Ok(bytes)
    }

    pub(crate) fn start(&mut self) -> Result<(), AdapterError> {
        self.sequence += 1;
        let device = self.device()?;
        device.start_emulating(self.converter.connection().serial(), self.sequence);
        self.flush()
    }

    /// Sends one evdev key event and waits for KWin to confirm it read it.
    pub(crate) fn key(
        &mut self,
        code: u32,
        down: bool,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        let device = self
            .keyboard
            .clone()
            .ok_or_else(|| refused("no keyboard"))?;
        let keyboard = device
            .interface::<ei::Keyboard>()
            .ok_or_else(|| refused("the KWin keyboard lost its key interface"))?;
        let state = if down {
            ei::keyboard::KeyState::Press
        } else {
            ei::keyboard::KeyState::Released
        };
        keyboard.key(code, state);
        device
            .device()
            .frame(self.converter.connection().serial(), now_micros());
        self.round_trip(deadline)
    }

    pub(crate) fn stop(&mut self) -> Result<(), AdapterError> {
        let device = self.device()?;
        device.stop_emulating(self.converter.connection().serial());
        self.flush()
    }

    fn device(&self) -> Result<ei::Device, AdapterError> {
        self.keyboard
            .as_ref()
            .map(|device| device.device().clone())
            .ok_or_else(|| refused("no keyboard"))
    }

    fn round_trip(&mut self, deadline: Deadline) -> Result<(), AdapterError> {
        let done = Rc::new(Cell::new(false));
        let callback = self
            .converter
            .connection()
            .connection()
            .sync(CALLBACK_VERSION);
        let seen = Rc::clone(&done);
        self.converter
            .add_callback_handler(callback, move |_| seen.set(true));
        self.flush()?;
        while !done.get() {
            if let Some(EiEvent::Disconnected(disconnected)) = self.step(deadline)? {
                return Err(refused(&format!("{:?}", disconnected.reason)));
            }
        }
        Ok(())
    }

    fn next_event(&mut self, deadline: Deadline) -> Result<EiEvent, AdapterError> {
        loop {
            if let Some(event) = self.step(deadline)? {
                return Ok(event);
            }
        }
    }

    fn step(&mut self, deadline: Deadline) -> Result<Option<EiEvent>, AdapterError> {
        if let Some(event) = self.converter.next_event() {
            return Ok(Some(event));
        }
        let mut handled = false;
        while let Some(result) = self.context.pending_event() {
            handled = true;
            let PendingRequestResult::Request(event) = result else {
                continue;
            };
            self.converter
                .handle_event(event)
                .map_err(|error| failure("event", &error))?;
        }
        if let Some(event) = self.converter.next_event() {
            return Ok(Some(event));
        }
        if !handled {
            read(&self.context, deadline)?;
        }
        Ok(None)
    }

    fn flush(&self) -> Result<(), AdapterError> {
        self.context
            .flush()
            .map_err(|error| failure("flush", &error))
    }
}

fn read(context: &ei::Context, deadline: Deadline) -> Result<(), AdapterError> {
    let remaining = deadline.remaining_slice(std::time::Duration::MAX)?;
    let timeout = Timespec::try_from(remaining).map_err(|error| failure("poll", &error))?;
    let fd = context.as_fd();
    let mut fds = [PollFd::new(&fd, PollFlags::IN)];
    let ready = poll(&mut fds, Some(&timeout)).map_err(|error| failure("poll", &error))?;
    if ready == 0 {
        return Err(deadline.timeout_error());
    }
    match context.read() {
        Ok(0) => Err(refused("KWin closed the input connection")),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(()),
        Err(error) => Err(failure("read", &error)),
    }
}

fn now_micros() -> u64 {
    let now = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
    u64::try_from(now.tv_sec).unwrap_or(0) * 1_000_000
        + u64::try_from(now.tv_nsec).unwrap_or(0) / 1_000
}

fn failure(stage: &str, error: &dyn std::fmt::Display) -> AdapterError {
    AdapterError::new(ErrorCode::ActionFailed, "The KWin input connection failed")
        .with_suggestion("Retry the command; report the platform detail if it persists")
        .with_platform_detail(format!("{stage}: {error}"))
}

fn refused(detail: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::ActionNotSupported,
        "Keyboard input through KWin is not available in this session",
    )
    .with_suggestion("Use the semantic action on the element instead of physical input")
    .with_platform_detail(detail.to_owned())
}
