use crate::types::window_op_kind::AdWindowOpKind;

#[repr(C)]
pub struct AdWindowOp {
    pub kind: AdWindowOpKind,
    pub width: f64,
    pub height: f64,
    pub x: f64,
    pub y: f64,
}
