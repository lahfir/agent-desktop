use crate::types::AdRect;
use std::os::raw::c_char;

#[repr(C)]
pub struct AdRefEntry {
    pub pid: i32,
    pub role: *const c_char,
    pub name: *const c_char,
    pub value: *const c_char,
    pub description: *const c_char,
    pub states: *const *const c_char,
    pub state_count: usize,
    pub available_actions: *const *const c_char,
    pub available_action_count: usize,
    pub bounds: AdRect,
    pub has_bounds: bool,
    pub bounds_hash: u64,
    pub has_bounds_hash: bool,
    pub source_app: *const c_char,
    pub source_window_id: *const c_char,
    pub source_window_title: *const c_char,
    pub source_surface: i32,
    pub root_ref: *const c_char,
    pub path_is_absolute: bool,
    pub path: *const u32,
    pub path_count: usize,
}

pub const AD_REF_ENTRY_SIZE: usize = 192;

const _: () = assert!(std::mem::size_of::<AdRefEntry>() == AD_REF_ENTRY_SIZE);

#[unsafe(no_mangle)]
pub extern "C" fn ad_ref_entry_size() -> usize {
    std::mem::size_of::<AdRefEntry>()
}
