mod config;
mod control;
mod hand_path;
mod instruction;
mod layout;
mod motion;
mod pose;
mod style;
mod submit;

pub use config::{CursorOverlayConfig, MAX_CURSOR_LABEL_WORDS};
pub use control::{CURSOR_OVERLAY_GREETING, CursorOverlayControl};
pub use instruction::CursorOverlayInstruction;
pub use layout::place_label;
pub use motion::CursorMotion;
pub use pose::CursorPose;
pub use style::CursorOverlayStyle;
pub(crate) use submit::submit;

#[cfg(test)]
mod tests;
