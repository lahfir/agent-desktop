use crate::types::snapshot_surface::AdSnapshotSurface;

#[repr(C)]
pub struct AdTreeOptions {
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: AdSnapshotSurface,
}
