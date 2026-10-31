//! The one seam that calls `SendMessageTimeoutW` for window-message input.
//!
//! Messages are sent, not posted, so each one has been processed by the
//! window's thread before the next leaves, and a window whose thread stops
//! responding ends the sequence at the timeout instead of blocking the
//! command. Test builds record into a thread-local fake, the same way
//! `keyboard_send.rs` does, so no unit test can deliver real input.

use std::time::Duration;

use agent_desktop_core::Deadline;

use crate::input::keyboard::keyboard_message::WindowMessage;

const PER_MESSAGE_TIMEOUT: Duration = Duration::from_millis(1_000);

/// Sends `messages` to `hwnd` in order and returns how many were delivered.
/// Delivery stops at the first message that times out or fails, and before
/// any message once `deadline` has expired.
pub(crate) fn send_window_messages(
    hwnd: isize,
    messages: &[WindowMessage],
    deadline: Deadline,
) -> usize {
    let mut delivered = 0;
    for message in messages {
        let remaining = deadline.remaining();
        if remaining.is_zero() {
            break;
        }
        let timeout = remaining.min(PER_MESSAGE_TIMEOUT);
        if !imp::send(hwnd, *message, timeout) {
            break;
        }
        delivered += 1;
    }
    delivered
}

#[cfg(target_os = "windows")]
mod imp {
    use std::time::Duration;

    use crate::input::keyboard::keyboard_message::WindowMessage;

    pub(super) fn send(hwnd: isize, message: WindowMessage, timeout: Duration) -> bool {
        #[cfg(test)]
        {
            let _ = timeout;
            super::window_message_fake_sink::record(&[(hwnd, message)]);
            true
        }
        #[cfg(not(test))]
        {
            native::send(hwnd, message, timeout)
        }
    }

    #[cfg(not(test))]
    mod native {
        use std::time::Duration;

        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SMTO_ABORTIFHUNG, SMTO_BLOCK, SendMessageTimeoutW,
        };

        use crate::input::keyboard::keyboard_message::WindowMessage;

        pub(super) fn send(hwnd: isize, message: WindowMessage, timeout: Duration) -> bool {
            let timeout_ms = u32::try_from(timeout.as_millis())
                .unwrap_or(u32::MAX)
                .max(1);
            let mut result = 0_usize;
            let sent = unsafe {
                SendMessageTimeoutW(
                    hwnd as _,
                    message.msg,
                    message.wparam,
                    message.lparam,
                    SMTO_ABORTIFHUNG | SMTO_BLOCK,
                    timeout_ms,
                    &mut result,
                )
            };
            sent != 0
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use std::time::Duration;

    use crate::input::keyboard::keyboard_message::WindowMessage;

    pub(super) fn send(_hwnd: isize, _message: WindowMessage, _timeout: Duration) -> bool {
        false
    }
}

crate::input::define_input_fake_sink!(
    window_message_fake_sink,
    (
        isize,
        crate::input::keyboard::keyboard_message::WindowMessage
    )
);
