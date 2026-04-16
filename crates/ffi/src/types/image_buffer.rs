use crate::types::image_format::AdImageFormat;

#[repr(C)]
pub struct AdImageBuffer {
    pub data: *const u8,
    pub data_len: u64,
    pub format: AdImageFormat,
    pub width: u32,
    pub height: u32,
}
