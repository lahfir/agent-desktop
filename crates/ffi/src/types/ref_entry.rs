use std::os::raw::c_char;

#[repr(C)]
pub struct AdRefEntry {
    pub pid: i32,
    pub role: *const c_char,
    pub name: *const c_char,
    pub bounds_hash: u64,
    pub has_bounds_hash: bool,
}
