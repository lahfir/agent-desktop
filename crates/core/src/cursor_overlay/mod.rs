mod config;
mod control;
mod instruction;
mod layout;
mod motion;
mod submit;

pub use config::{CursorOverlayConfig, MAX_CURSOR_LABEL_WORDS};
pub use control::{CURSOR_OVERLAY_GREETING, CursorOverlayControl};
pub use instruction::CursorOverlayInstruction;
pub use layout::place_label;
pub use motion::CursorMotion;
pub(crate) use submit::submit;

#[cfg(test)]
mod tests;
