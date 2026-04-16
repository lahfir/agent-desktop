use crate::types::point::AdPoint;

#[repr(C)]
pub struct AdDragParams {
    pub from: AdPoint,
    pub to: AdPoint,
    pub duration_ms: u64,
}
