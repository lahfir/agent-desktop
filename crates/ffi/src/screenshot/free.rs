use crate::types::AdImageBuffer;

/// # Safety
/// `img` must be null or point to an `AdImageBuffer` from `ad_screenshot`.
#[no_mangle]
pub unsafe extern "C" fn ad_free_image(img: *mut AdImageBuffer) {
    if img.is_null() {
        return;
    }
    let i = &mut *img;
    if !i.data.is_null() {
        drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(
            i.data as *mut u8,
            i.data_len as usize,
        )));
        i.data = std::ptr::null();
        i.data_len = 0;
    }
}
