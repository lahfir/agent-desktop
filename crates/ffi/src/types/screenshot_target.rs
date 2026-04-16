/// Screenshot target for `ad_screenshot`.
///
/// `kind` is stored as `int32_t` to keep the enum-discriminant check
/// at the boundary. Valid values are the discriminants of
/// `AdScreenshotKind`. `screen_index` is only consulted when kind is
/// `SCREEN`; `pid` only when kind is `WINDOW`.
#[repr(C)]
pub struct AdScreenshotTarget {
    pub kind: i32,
    pub screen_index: u64,
    pub pid: i32,
}
