use crate::snapshot_surface::SnapshotSurface;

#[derive(Clone, Copy)]
pub struct TreeOptions {
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
    pub skeleton: bool,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            max_depth: 10,
            include_bounds: false,
            interactive_only: false,
            compact: false,
            surface: SnapshotSurface::Window,
            skeleton: false,
        }
    }
}

impl TreeOptions {
    pub(crate) fn with_ref_identity_bounds(mut self) -> Self {
        self.include_bounds = true;
        self
    }
}
