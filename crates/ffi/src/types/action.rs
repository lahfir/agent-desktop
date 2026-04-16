use crate::types::action_kind::AdActionKind;
use crate::types::drag_params::AdDragParams;
use crate::types::key_combo::AdKeyCombo;
use crate::types::scroll_params::AdScrollParams;
use std::os::raw::c_char;

#[repr(C)]
pub struct AdAction {
    pub kind: AdActionKind,
    pub text: *const c_char,
    pub scroll: AdScrollParams,
    pub key: AdKeyCombo,
    pub drag: AdDragParams,
}
