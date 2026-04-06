use std::os::raw::c_char;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AdRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AdPoint {
    pub x: f64,
    pub y: f64,
}

#[repr(C)]
pub struct AdWindowInfo {
    pub id: *const c_char,
    pub title: *const c_char,
    pub app_name: *const c_char,
    pub pid: i32,
    pub bounds: AdRect,
    pub has_bounds: bool,
    pub is_focused: bool,
}

#[repr(C)]
pub struct AdAppInfo {
    pub name: *const c_char,
    pub pid: i32,
    pub bundle_id: *const c_char,
}

#[repr(C)]
pub struct AdSurfaceInfo {
    pub kind: *const c_char,
    pub title: *const c_char,
    pub item_count: i64,
}
