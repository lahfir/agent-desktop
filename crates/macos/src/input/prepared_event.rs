use core_graphics::event::CGEvent;
use std::time::Duration;

/// One built, not yet posted, event of a background delivery.
///
/// `completes_press` marks a button-up or key-up that releases a press
/// posted just before it. The delivery loop always posts those, even after
/// the deadline expired, so a partial delivery never leaves a button or key
/// held down in the target; every other event is posted only while budget
/// remains.
pub(crate) struct PreparedEvent {
    pub(crate) event: CGEvent,
    pub(crate) pause_after: Duration,
    pub(crate) completes_press: bool,
}
