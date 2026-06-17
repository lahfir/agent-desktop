use std::os::raw::c_char;

#[repr(C)]
pub struct AdActionStep {
    pub label: *const c_char,
    pub outcome: *const c_char,
}

const _: () = assert!(std::mem::size_of::<AdActionStep>() == 16);
