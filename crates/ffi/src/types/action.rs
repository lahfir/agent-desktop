use crate::types::drag_params::AdDragParams;
use crate::types::key_combo::AdKeyCombo;
use crate::types::scroll_params::AdScrollParams;
use std::os::raw::c_char;

/// Action dispatched by `ad_execute_action`.
///
/// `kind` is stored as `int32_t` so a buggy or malicious C caller
/// cannot write an out-of-range discriminant into a Rust enum slot —
/// an out-of-range value is rejected with
/// `AD_RESULT_ERR_INVALID_ARGS` at the boundary. Valid values are the
/// discriminants of `AdActionKind`.
#[repr(C)]
pub struct AdAction {
    pub kind: i32,
    pub text: *const c_char,
    pub scroll: AdScrollParams,
    pub key: AdKeyCombo,
    pub drag: AdDragParams,
}
