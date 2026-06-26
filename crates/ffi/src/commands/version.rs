use crate::commands::envelope_out::write_command_envelope;
use crate::error::AdResult;
use crate::ffi_try::trap_panic;
use crate::pointer_guard::guard_non_null;
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
        guard_non_null!(out, c"out is null");
        *out = std::ptr::null_mut();
        let result = agent_desktop_core::commands::version::execute();
        write_command_envelope("version", result, out)
    })
}
