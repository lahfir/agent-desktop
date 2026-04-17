use crate::types::app_info::AdAppInfo;

/// Opaque list handle emitted by `ad_list_apps`. See
/// [`crate::types::window_list::AdWindowList`] for the pattern.
pub struct AdAppList {
    pub(crate) items: Box<[AdAppInfo]>,
}
