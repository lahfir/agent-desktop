mod config;
mod control;
mod cue;
mod hand_path;
mod instruction;
mod layout;
mod motion;
mod phase;
mod pose;
mod style;
mod submit;
mod timing;

pub use config::{CursorOverlayConfig, MAX_CURSOR_LABEL_WORDS};
pub use control::{CURSOR_OVERLAY_GREETING, CursorOverlayControl};
pub(crate) use cue::{PointerCue, presentation_window};
pub use instruction::CursorOverlayInstruction;
pub use layout::place_label;
pub use motion::CursorMotion;
pub use phase::CursorPhase;
pub use pose::CursorPose;
pub use style::CursorOverlayStyle;
pub(crate) use submit::{
    cancel_drag, confirms_delivery, dispatch_mouse_event_with_cursor, input_was_delivered, send,
    submit, submit_drag, submit_drag_effect, submit_travel, travel_scope,
};
pub use timing::{CURSOR_ARRIVAL_TIMEOUT_MS, CURSOR_HIGHLIGHT_HOLD_MS, CURSOR_IDLE_REST_MS};

#[cfg(test)]
mod routing_tests;
#[cfg(test)]
mod tests;
