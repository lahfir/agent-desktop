use crate::types::rect::AdRect;
use std::os::raw::c_char;

#[repr(C)]
pub struct AdNode {
    pub ref_id: *const c_char,
    pub role: *const c_char,
    pub name: *const c_char,
    pub value: *const c_char,
    pub description: *const c_char,
    pub hint: *const c_char,
    pub states: *mut *mut c_char,
    pub state_count: u32,
    pub bounds: AdRect,
    pub has_bounds: bool,
    pub parent_index: i32,
    pub child_start: u32,
    pub child_count: u32,
}
