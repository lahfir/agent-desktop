use crate::error::{AdResult, set_last_error_static};
use crate::ffi_try::trap_panic;
use std::ffi::CStr;

/// The major ABI version of this build of `libagent_desktop_ffi`.
///
/// Version-bump rule: increment this constant (and update the header via
/// `scripts/update-ffi-header.sh`) whenever a breaking change is made to the
/// C ABI — a removed or incompatibly-changed `ad_*` symbol, or a layout
/// change to any `repr(C)` struct. Additive changes (new `ad_*` symbols, new
/// error codes) do **not** require a bump. Consumers must call `ad_init` with
/// the major they compiled against before making any adapter calls; a mismatch
/// returns `AD_RESULT_ERR_INVALID_ARGS` so they can refuse gracefully rather
/// than corrupt memory.
pub const AD_ABI_VERSION_MAJOR: u32 = 1;

static MISMATCH_MESSAGE: &CStr =
    c"ABI major version mismatch: recompile against the installed header";

/// Returns the packed ABI major version of this dylib build.
///
/// A consumer should compare this to `AD_ABI_VERSION_MAJOR` from the header it
/// compiled against. If they differ, call nothing further — the ABI is
/// incompatible.
#[unsafe(no_mangle)]
pub extern "C" fn ad_abi_version() -> u32 {
    AD_ABI_VERSION_MAJOR
}

/// Validates that the consumer's expected ABI major matches this dylib.
///
/// Call once after `dlopen` / `LoadLibrary`, before any adapter call.
/// Returns `AD_RESULT_OK` when `expected_major == AD_ABI_VERSION_MAJOR`.
/// Returns `AD_RESULT_ERR_INVALID_ARGS` with a diagnostic last-error when the
/// version does not match, so the consumer can refuse to proceed rather than
/// crash with an incompatible ABI.
#[unsafe(no_mangle)]
pub extern "C" fn ad_init(expected_major: u32) -> AdResult {
    trap_panic(|| {
        if expected_major == AD_ABI_VERSION_MAJOR {
            AdResult::Ok
        } else {
            set_last_error_static(AdResult::ErrInvalidArgs, MISMATCH_MESSAGE);
            AdResult::ErrInvalidArgs
        }
    })
}
