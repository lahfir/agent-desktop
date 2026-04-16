use crate::types::modifier::AdModifier;
use std::os::raw::c_char;

#[repr(C)]
pub struct AdKeyCombo {
    pub key: *const c_char,
    pub modifiers: *const AdModifier,
    pub modifier_count: u32,
}
