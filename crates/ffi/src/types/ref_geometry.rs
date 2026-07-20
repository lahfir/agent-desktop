use crate::types::AdRect;

#[repr(C)]
pub struct AdRefGeometry {
    pub bounds: AdRect,
    pub bounds_hash: u64,
    pub has_bounds: bool,
    pub has_bounds_hash: bool,
}

pub const AD_REF_GEOMETRY_SIZE: usize = 48;

const _: () = assert!(std::mem::size_of::<AdRefGeometry>() == AD_REF_GEOMETRY_SIZE);
