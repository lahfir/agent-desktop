use crate::enum_validation::enum_raw_i32;
use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::{AdImageBuffer, AdImageFormat, AdScreenshotKind, AdScreenshotTarget};
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
    trap_panic(|| unsafe {
        *out = std::mem::zeroed();
        let adapter = &*adapter;
        let t = &*target;
        let kind = match AdScreenshotKind::from_c(enum_raw_i32(&t.kind)) {
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
    })
}
