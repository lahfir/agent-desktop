use crate::types::element_state::AdElementState;
use std::os::raw::c_char;

#[repr(C)]
pub struct AdActionResult {
    pub action: *const c_char,
    pub ref_id: *const c_char,
    pub post_state: *mut AdElementState,
}
