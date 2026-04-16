use crate::types::window_info::AdWindowInfo;

/// Opaque list handle emitted by `ad_list_windows`.
///
/// The struct intentionally has no `#[repr(C)]` so cbindgen emits a
/// forward declaration only (`typedef struct AdWindowList AdWindowList;`).
/// Consumers cannot read the backing pointer or length and cannot
/// construct a count mismatch — they walk the list through
/// `ad_window_list_count`, `ad_window_list_get`, and free it with
/// `ad_window_list_free`.
pub struct AdWindowList {
    pub(crate) items: Box<[AdWindowInfo]>,
}
