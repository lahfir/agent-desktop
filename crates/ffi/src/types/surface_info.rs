use std::os::raw::c_char;

#[repr(C)]
pub struct AdSurfaceInfo {
    pub kind: *const c_char,
    pub title: *const c_char,
    pub item_count: i64,
}
