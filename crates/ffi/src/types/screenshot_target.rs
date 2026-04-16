use crate::types::screenshot_kind::AdScreenshotKind;

#[repr(C)]
pub struct AdScreenshotTarget {
    pub kind: AdScreenshotKind,
    pub screen_index: u64,
    pub pid: i32,
}
