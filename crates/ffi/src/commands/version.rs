use crate::convert::string::string_to_c;
use crate::error::{self, AdResult};
use crate::ffi_try::trap_panic;
use agent_desktop_core::output::Response;
use std::os::raw::c_char;

/// Returns the `agent-desktop` version envelope as an owned JSON C string.
///
/// The returned string has the same `{version, ok, command, data}` shape
/// as `agent-desktop version` on the CLI. Free it with `ad_free_string`.
///
/// On success `*out` points to the envelope JSON.
/// On error `*out` is null and the last-error slot is populated.
///
/// # Safety
/// `out` must be a non-null writable `*mut *mut c_char`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_version(out: *mut *mut c_char) -> AdResult {
    trap_panic(|| unsafe {
        crate::pointer_guard::guard_non_null!(out, c"out is null");
        *out = std::ptr::null_mut();
        match agent_desktop_core::commands::version::execute() {
            Ok(data) => {
                let envelope = Response::ok("version", data);
                let json = match serde_json::to_string(&envelope) {
                    Ok(s) => s,
                    Err(e) => {
                        error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                            agent_desktop_core::error::ErrorCode::Internal,
                            format!("failed to serialize version envelope: {e}"),
                        ));
                        return AdResult::ErrInternal;
                    }
                };
                let c = string_to_c(&json);
                if c.is_null() {
                    error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                        agent_desktop_core::error::ErrorCode::Internal,
                        "version JSON contains an interior NUL byte",
                    ));
                    return AdResult::ErrInternal;
                }
                *out = c;
                AdResult::Ok
            }
            Err(e) => {
                error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::Internal,
                    e.to_string(),
                ));
                AdResult::ErrInternal
            }
        }
    })
}
