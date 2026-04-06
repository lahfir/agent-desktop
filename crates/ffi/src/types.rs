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

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdActionKind {
    Click = 0,
    DoubleClick = 1,
    RightClick = 2,
    TripleClick = 3,
    SetValue = 4,
    SetFocus = 5,
    Expand = 6,
    Collapse = 7,
    Select = 8,
    Toggle = 9,
    Check = 10,
    Uncheck = 11,
    Scroll = 12,
    ScrollTo = 13,
    PressKey = 14,
    KeyDown = 15,
    KeyUp = 16,
    TypeText = 17,
    Clear = 18,
    Hover = 19,
    Drag = 20,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdDirection {
    Up = 0,
    Down = 1,
    Left = 2,
    Right = 3,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdModifier {
    Cmd = 0,
    Ctrl = 1,
    Alt = 2,
    Shift = 3,
}

#[repr(C)]
pub struct AdKeyCombo {
    pub key: *const c_char,
    pub modifiers: *const AdModifier,
    pub modifier_count: u32,
}

#[repr(C)]
pub struct AdDragParams {
    pub from: AdPoint,
    pub to: AdPoint,
    pub duration_ms: u64,
}

#[repr(C)]
pub struct AdScrollParams {
    pub direction: AdDirection,
    pub amount: u32,
}

#[repr(C)]
pub struct AdAction {
    pub kind: AdActionKind,
    pub text: *const c_char,
    pub scroll: AdScrollParams,
    pub key: AdKeyCombo,
    pub drag: AdDragParams,
}

#[repr(C)]
pub struct AdElementState {
    pub role: *const c_char,
    pub states: *mut *mut c_char,
    pub state_count: u32,
    pub value: *const c_char,
}

#[repr(C)]
pub struct AdActionResult {
    pub action: *const c_char,
    pub ref_id: *const c_char,
    pub post_state: *mut AdElementState,
}

#[repr(C)]
pub struct AdNativeHandle {
    pub ptr: *const std::ffi::c_void,
}

#[repr(C)]
pub struct AdRefEntry {
    pub pid: i32,
    pub role: *const c_char,
    pub name: *const c_char,
    pub bounds_hash: u64,
    pub has_bounds_hash: bool,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdWindowOpKind {
    Resize = 0,
    Move = 1,
    Minimize = 2,
    Maximize = 3,
    Restore = 4,
}

#[repr(C)]
pub struct AdWindowOp {
    pub kind: AdWindowOpKind,
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdMouseButton {
    Left = 0,
    Right = 1,
    Middle = 2,
}

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdMouseEventKind {
    Move = 0,
    Down = 1,
    Up = 2,
    Click = 3,
}

#[repr(C)]
pub struct AdMouseEvent {
    pub kind: AdMouseEventKind,
    pub point: AdPoint,
    pub button: AdMouseButton,
    pub click_count: u32,
}
