use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::AdNativeHandle;
use crate::AdAdapter;
use agent_desktop_core::adapter::NativeHandle;

/// Releases a handle previously returned by `ad_resolve_element` and
/// zeroes the caller's struct so accidentally calling this twice is
/// a deterministic no-op instead of a double-free on the underlying
/// `CFRelease`.
///
/// On macOS this calls `CFRelease` on the underlying `AXUIElementRef`,
/// balancing the `CFRetain` that happened during `ad_resolve_element`.
/// On Windows/Linux the call is a no-op that returns `AD_RESULT_OK`
/// (platform adapters inherit the default `not_supported` impl; the
/// FFI surface translates it so callers apply the same release
/// pattern everywhere).
///
/// Ownership contract: the FFI owns the handle from the moment
/// `ad_resolve_element` writes `ptr`. Copying the struct after that
/// point and calling `ad_free_handle` on either copy is undefined —
/// there is no way for the library to detect forged non-null pointers.
/// Callers that legitimately need a "copy" should re-resolve.
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `handle` must be null or a `*mut AdNativeHandle` previously
/// populated by `ad_resolve_element`. On return `(*handle).ptr` is
/// `NULL` so a double-call is a no-op instead of a double-free.
#[no_mangle]
pub unsafe extern "C" fn ad_free_handle(
    adapter: *const AdAdapter,
    handle: *mut AdNativeHandle,
) -> AdResult {
    trap_panic(|| unsafe {
        if adapter.is_null() {
            set_last_error(&agent_desktop_core::error::AdapterError::new(
                agent_desktop_core::error::ErrorCode::InvalidArgs,
                "adapter is null",
            ));
            return AdResult::ErrInvalidArgs;
        }
        if handle.is_null() {
            return AdResult::Ok;
        }
        let raw = (*handle).ptr;
        if raw.is_null() {
            return AdResult::Ok;
        }
        // Zero the caller-visible pointer *before* the platform release
        // so a concurrent or accidental double-call through the same
        // struct cannot re-enter CFRelease on the same underlying ref.
        (*handle).ptr = std::ptr::null();
        let adapter = &*adapter;
        let native = NativeHandle::from_ptr(raw);
        match adapter.inner.release_handle(&native) {
            Ok(()) => AdResult::Ok,
            Err(e) => {
                if matches!(
                    e.code,
                    agent_desktop_core::error::ErrorCode::ActionNotSupported
                        | agent_desktop_core::error::ErrorCode::PlatformNotSupported
                ) {
                    return AdResult::Ok;
                }
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
