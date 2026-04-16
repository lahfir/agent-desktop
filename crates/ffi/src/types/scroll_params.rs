use crate::types::direction::AdDirection;

#[repr(C)]
pub struct AdScrollParams {
    pub direction: AdDirection,
    pub amount: u32,
}
