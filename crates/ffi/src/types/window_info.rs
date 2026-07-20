use crate::types::rect::AdRect;
use std::os::raw::c_char;

#[repr(C)]
pub struct AdWindowInfo {
    /// Legacy observation-only window ID. This struct has no process-generation
    /// evidence and is rejected by targeting APIs; use `AdExactWindowInfo` for
    /// any operation that sends a previously observed window back to the library.
    pub id: *const c_char,
    pub title: *const c_char,
    pub app_name: *const c_char,
    pub pid: u32,
    pub bounds: AdRect,
    pub has_bounds: bool,
    pub is_focused: bool,
}
