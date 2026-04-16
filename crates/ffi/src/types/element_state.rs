use std::os::raw::c_char;

#[repr(C)]
pub struct AdElementState {
    pub role: *const c_char,
    pub states: *mut *mut c_char,
    pub state_count: u32,
    pub value: *const c_char,
}
