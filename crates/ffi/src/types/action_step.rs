use std::os::raw::c_char;

#[repr(C)]
pub struct AdActionStep {
    pub label: *const c_char,
    pub outcome: *const c_char,
}

pub const AD_ACTION_STEP_SIZE: usize = 16;

const _: () = assert!(std::mem::size_of::<AdActionStep>() == AD_ACTION_STEP_SIZE);

#[unsafe(no_mangle)]
pub extern "C" fn ad_action_step_size() -> usize {
    std::mem::size_of::<AdActionStep>()
}
