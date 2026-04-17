use crate::error::{self, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_ptr, trap_panic_void};
use agent_desktop_core::adapter::PlatformAdapter;

pub struct AdAdapter {
    pub(crate) inner: Box<dyn PlatformAdapter>,
}

fn build_adapter() -> Box<dyn PlatformAdapter> {
    #[cfg(target_os = "macos")]
    {
        Box::new(agent_desktop_macos::MacOSAdapter::new())
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(agent_desktop_windows::WindowsAdapter::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(agent_desktop_linux::LinuxAdapter::new())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    compile_error!("Unsupported platform")
}

/// Builds a platform adapter for the current OS and returns an opaque
/// handle. Returns null on allocation failure or if a Rust panic is
/// caught at the FFI boundary (inspect `ad_last_error_*` for details).
///
/// The returned pointer is owned by the caller and must be released with
/// `ad_adapter_destroy`. Creating and destroying adapters is cheap; the
/// common pattern is one adapter per process lifetime.
#[no_mangle]
pub extern "C" fn ad_adapter_create() -> *mut AdAdapter {
    trap_panic_ptr(|| {
        let adapter = AdAdapter {
            inner: build_adapter(),
        };
        Box::into_raw(Box::new(adapter))
    })
}

/// # Safety
///
/// `adapter` must be a pointer returned by `ad_adapter_create`, or null.
/// After this call the pointer is invalid and must not be used.
#[no_mangle]
pub unsafe extern "C" fn ad_adapter_destroy(adapter: *mut AdAdapter) {
    trap_panic_void(|| {
        if !adapter.is_null() {
            drop(unsafe { Box::from_raw(adapter) });
        }
    })
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create` that
/// has not yet been destroyed.
#[no_mangle]
pub unsafe extern "C" fn ad_check_permissions(adapter: *const AdAdapter) -> AdResult {
    trap_panic(|| {
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = unsafe { &*adapter };
        match adapter.inner.check_permissions() {
            agent_desktop_core::adapter::PermissionStatus::Granted => AdResult::Ok,
            agent_desktop_core::adapter::PermissionStatus::Denied { suggestion } => {
                error::set_last_error(
                    &agent_desktop_core::error::AdapterError::new(
                        agent_desktop_core::error::ErrorCode::PermDenied,
                        "Accessibility permission not granted",
                    )
                    .with_suggestion(suggestion),
                );
                AdResult::ErrPermDenied
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_create_destroy() {
        let ptr = ad_adapter_create();
        assert!(!ptr.is_null());
        unsafe { ad_adapter_destroy(ptr) };
    }

    #[test]
    fn test_destroy_null_is_noop() {
        unsafe { ad_adapter_destroy(std::ptr::null_mut()) };
    }
}
