use crate::types::mouse_button::AdMouseButton;
use crate::types::mouse_event_kind::AdMouseEventKind;
use crate::types::point::AdPoint;

#[repr(C)]
pub struct AdMouseEvent {
    pub kind: AdMouseEventKind,
    pub point: AdPoint,
    pub button: AdMouseButton,
    pub click_count: u32,
}
