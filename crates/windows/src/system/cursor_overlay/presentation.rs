//! What one composed frame is made of, and whether the overlay has gone quiet.
//!
//! Held apart from the host so the host file stays inside the size cap and so
//! the two pieces of per-frame state can be read without the paint path around
//! them.

#[cfg(target_os = "windows")]
pub(crate) use imp::{Presentation, RestState};

#[cfg(target_os = "windows")]
mod imp {
    use crate::system::cursor_overlay::reveal::Reveal;
    use agent_desktop_core::{CursorOverlayStyle, Point};
    use std::time::Instant;

    /// The three things a frame is composed from, plus when the card began
    /// appearing.
    ///
    /// The reveal instant belongs here rather than beside the rest state
    /// because it is a property of the label: it is set when the label
    /// changes and read while the card is drawn.
    pub(crate) struct Presentation {
        pub(crate) style: CursorOverlayStyle,
        pub(crate) pose: Point,
        pub(crate) label: Option<String>,
        pub(crate) reveal: Reveal,
    }

    /// Whether the overlay has faded out, and how long it has been quiet.
    ///
    /// The overlay does not sit on screen indefinitely after the last
    /// instruction: it fades and orders itself away, and the next control
    /// brings it straight back at full strength.
    pub(crate) struct RestState {
        pub(crate) resting: bool,
        pub(crate) quiet_since: Instant,
    }
}
