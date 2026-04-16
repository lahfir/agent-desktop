use crate::types::surface_info::AdSurfaceInfo;

/// Opaque list handle emitted by `ad_list_surfaces`. See
/// [`crate::types::window_list::AdWindowList`] for the pattern.
pub struct AdSurfaceList {
    pub(crate) items: Box<[AdSurfaceInfo]>,
}
