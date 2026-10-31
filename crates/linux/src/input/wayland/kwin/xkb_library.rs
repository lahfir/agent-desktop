use std::ffi::{CStr, c_void};

use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};
use serde_json::json;

const LIBRARY: &CStr = c"libxkbcommon.so.0";

/// libxkbcommon, loaded at run time so the binary links nothing beyond libc
/// and only a KWin keyboard delivery needs the library present.
pub(crate) struct XkbLibrary {
    handle: *mut c_void,
}

impl XkbLibrary {
    pub(crate) fn open() -> Result<Self, AdapterError> {
        let handle = unsafe { libc::dlopen(LIBRARY.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        if handle.is_null() {
            return Err(unavailable("libxkbcommon.so.0 could not be loaded"));
        }
        Ok(Self { handle })
    }

    /// The function `name` exports, typed as `T`.
    ///
    /// # Safety
    ///
    /// `T` must be the `extern "C"` function pointer type of `name`.
    pub(crate) unsafe fn symbol<T: Copy>(&self, name: &CStr) -> Result<T, AdapterError> {
        let address = unsafe { libc::dlsym(self.handle, name.as_ptr()) };
        if address.is_null() {
            return Err(unavailable(&format!(
                "libxkbcommon exports no {}",
                name.to_string_lossy()
            )));
        }
        Ok(unsafe { std::mem::transmute_copy::<*mut c_void, T>(&address) })
    }
}

impl Drop for XkbLibrary {
    fn drop(&mut self) {
        unsafe {
            libc::dlclose(self.handle);
        }
    }
}

pub(crate) fn unavailable(detail: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::PlatformNotSupported,
        "The KWin keyboard layout could not be read in this Wayland session",
    )
    .with_platform_detail(detail.to_owned())
    .with_suggestion("Install libxkbcommon, or use the semantic action on the element")
    .with_details(json!({ "physical_delivery_started": false }))
    .with_disposition(DeliverySemantics::not_delivered())
}
