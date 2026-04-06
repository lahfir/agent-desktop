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

#[repr(C)]
pub struct AdNodeTree {
    pub nodes: *mut AdNode,
    pub count: u32,
}

#[repr(C)]
pub struct AdTreeOptions {
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
}
