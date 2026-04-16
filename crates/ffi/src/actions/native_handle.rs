use crate::error::{set_last_error, AdResult};
use crate::ffi_try::trap_panic;
use crate::types::AdNativeHandle;
use crate::AdAdapter;
use agent_desktop_core::adapter::NativeHandle;

/// Releases a handle previously returned by `ad_resolve_element`.
///
/// On macOS this calls `CFRelease` on the underlying `AXUIElementRef`,
/// balancing the `CFRetain` that happened during `ad_resolve_element`.
/// On Windows/Linux the call is a no-op that returns `AD_RESULT_OK`
/// (platform adapters inherit the default `not_supported` impl, which
/// the FFI surface rewrites to `Ok` here so callers can apply the same
/// release pattern everywhere).
///
/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create`.
/// `handle` must be null or a pointer previously populated by
/// `ad_resolve_element`. Double-free is undefined behavior.
#[no_mangle]
pub unsafe extern "C" fn ad_free_handle(
    adapter: *const AdAdapter,
    handle: *const AdNativeHandle,
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
        let adapter = &*adapter;
        let raw = (*handle).ptr;
        if raw.is_null() {
            return AdResult::Ok;
        }
        let native = NativeHandle::from_ptr(raw);
        match adapter.inner.release_handle(&native) {
            Ok(()) => AdResult::Ok,
            Err(e) => {
                // Not-supported on Windows/Linux is a no-op by contract.
                if matches!(
                    e.code,
                    agent_desktop_core::error::ErrorCode::ActionNotSupported
                ) {
                    return AdResult::Ok;
                }
                set_last_error(&e);
                crate::error::last_error_code()
            }
        }
    })
}
