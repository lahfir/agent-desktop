mod config;
mod control;
mod hand_path;
mod instruction;
mod layout;
mod motion;
mod phase;
mod pose;
mod style;
mod submit;
mod timing;

pub use config::{CursorOverlayConfig, MAX_CURSOR_LABEL_BYTES, MAX_CURSOR_LABEL_WORDS};
pub use control::{CURSOR_OVERLAY_GREETING, CursorOverlayControl};
pub use instruction::CursorOverlayInstruction;
pub use layout::place_label;
pub use motion::CursorMotion;
pub use phase::CursorPhase;
pub use pose::CursorPose;
pub use style::CursorOverlayStyle;
pub(crate) use submit::{
    cancel_drag, confirms_delivery, dispatch_mouse_event_with_cursor, input_was_delivered, submit,
    submit_drag, submit_drag_effect, submit_travel,
};
pub use timing::{
    CURSOR_ARRIVAL_TIMEOUT_MS, CURSOR_HIGHLIGHT_HOLD_MS, CURSOR_IDLE_REST_MS,
    CURSOR_LABEL_REVEAL_MS, CURSOR_REST_FADE_MS,
};

#[cfg(test)]
mod routing_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
#[path = "greeting_tests.rs"]
mod greeting_tests;
