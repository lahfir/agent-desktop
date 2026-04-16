use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdImageBuffer, AdImageFormat, AdScreenshotKind, AdScreenshotTarget};
use crate::AdAdapter;
use agent_desktop_core::adapter::{ImageFormat, ScreenshotTarget as CoreScreenshotTarget};
use std::ptr;

/// Allocates and returns an opaque `AdImageBuffer`. The handle owns its
/// byte buffer; inspect it through `ad_image_buffer_data` /
/// `ad_image_buffer_size` / `ad_image_buffer_format` / `_width` / `_height`
/// and free it with `ad_image_buffer_free`.
///
/// # Safety
/// `adapter` and `target` must be valid pointers. `out` must be a valid
/// writable `*mut *mut AdImageBuffer`. On error `*out` is null and
/// last-error is set.
#[no_mangle]
pub unsafe extern "C" fn ad_screenshot(
    adapter: *const AdAdapter,
    target: *const AdScreenshotTarget,
    out: *mut *mut AdImageBuffer,
) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = ptr::null_mut();
        if let Err(rc) = crate::main_thread::require_main_thread() {
            return rc;
        }
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        crate::pointer_guard::guard_non_null!(target, c"target is null");
        let adapter = &*adapter;
        let t = &*target;
        let kind = match AdScreenshotKind::from_c(t.kind) {
            Some(k) => k,
            None => {
                set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    "invalid screenshot kind discriminant",
                ));
                return AdResult::ErrInvalidArgs;
            }
        };
        let core_target = match kind {
            AdScreenshotKind::Screen => CoreScreenshotTarget::Screen(t.screen_index as usize),
            AdScreenshotKind::Window => CoreScreenshotTarget::Window(t.pid),
            AdScreenshotKind::FullScreen => CoreScreenshotTarget::FullScreen,
        };

        match adapter.inner.screenshot(core_target) {
            Ok(img) => {
                let buffer = Box::new(AdImageBuffer {
                    data: img.data.into_boxed_slice(),
                    width: img.width,
                    height: img.height,
                    format: match img.format {
                        ImageFormat::Png => AdImageFormat::Png,
                        ImageFormat::Jpg => AdImageFormat::Jpg,
                    },
                });
                *out = Box::into_raw(buffer);
                AdResult::Ok
            }
            Err(e) => {
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
