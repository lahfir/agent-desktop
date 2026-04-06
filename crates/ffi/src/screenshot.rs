use crate::error::{clear_last_error, set_last_error, AdResult};
use crate::types::*;
use crate::AdAdapter;
use agent_desktop_core::adapter::{ImageFormat, ScreenshotTarget as CoreScreenshotTarget};

/// # Safety
/// `adapter` and `target` must be valid. `out` must be writable.
#[no_mangle]
pub unsafe extern "C" fn ad_screenshot(
    adapter: *const AdAdapter,
    target: *const AdScreenshotTarget,
    out: *mut AdImageBuffer,
) -> AdResult {
    let adapter = &*adapter;
    let t = &*target;
    let core_target = match t.kind {
        AdScreenshotKind::Screen => CoreScreenshotTarget::Screen(t.screen_index as usize),
        AdScreenshotKind::Window => CoreScreenshotTarget::Window(t.pid),
        AdScreenshotKind::FullScreen => CoreScreenshotTarget::FullScreen,
    };

    match adapter.inner.screenshot(core_target) {
        Ok(img) => {
            clear_last_error();
            let data_len = img.data.len() as u64;
            let mut boxed = img.data.into_boxed_slice();
            let data_ptr = boxed.as_mut_ptr();
            std::mem::forget(boxed);

            *out = AdImageBuffer {
                data: data_ptr,
                data_len,
                format: match img.format {
                    ImageFormat::Png => AdImageFormat::Png,
                    ImageFormat::Jpg => AdImageFormat::Jpg,
                },
                width: img.width,
                height: img.height,
            };
            AdResult::Ok
        }
        Err(e) => {
            set_last_error(&e);
            crate::error::last_error_code()
        }
    }
}

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
