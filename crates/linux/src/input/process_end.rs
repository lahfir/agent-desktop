use super::wayland::kept_session;
use super::x11::key_settle;

/// Ends this process's input before it exits. It stays on the accessibility
/// bus until the last X11 key it sent has been handled, then stops the kept
/// Wayland keyboard session.
pub fn end() {
    key_settle::wait_for_last_key();
    kept_session::end();
}
