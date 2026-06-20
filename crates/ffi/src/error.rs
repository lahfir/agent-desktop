use agent_desktop_core::error::{AdapterError, ErrorCode};
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char};

const fn error_code_variant_count() -> usize {
    let variants = [
        ErrorCode::PermDenied,
        ErrorCode::ElementNotFound,
        ErrorCode::AppNotFound,
        ErrorCode::ActionFailed,
        ErrorCode::ActionNotSupported,
        ErrorCode::StaleRef,
        ErrorCode::AmbiguousTarget,
        ErrorCode::WindowNotFound,
        ErrorCode::PlatformNotSupported,
        ErrorCode::Timeout,
        ErrorCode::InvalidArgs,
        ErrorCode::NotificationNotFound,
        ErrorCode::SnapshotNotFound,
        ErrorCode::PolicyDenied,
        ErrorCode::Internal,
    ];
    variants.len()
}

const fn ad_result_error_variant_count() -> usize {
    let variants = [
        AdResult::ErrPermDenied,
        AdResult::ErrElementNotFound,
        AdResult::ErrAppNotFound,
        AdResult::ErrActionFailed,
        AdResult::ErrActionNotSupported,
        AdResult::ErrStaleRef,
        AdResult::ErrAmbiguousTarget,
        AdResult::ErrWindowNotFound,
        AdResult::ErrPlatformNotSupported,
        AdResult::ErrTimeout,
        AdResult::ErrInvalidArgs,
        AdResult::ErrNotificationNotFound,
        AdResult::ErrInternal,
        AdResult::ErrSnapshotNotFound,
        AdResult::ErrPolicyDenied,
    ];
    variants.len()
}

const _: () = assert!(
    error_code_variant_count() == ad_result_error_variant_count(),
    "ErrorCode variants must match AdResult error-code variants one-to-one"
);

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdResult {
    Ok = 0,
    ErrPermDenied = -1,
    ErrElementNotFound = -2,
    ErrAppNotFound = -3,
    ErrActionFailed = -4,
    ErrActionNotSupported = -5,
    ErrStaleRef = -6,
    ErrWindowNotFound = -7,
    ErrPlatformNotSupported = -8,
    ErrTimeout = -9,
    ErrInvalidArgs = -10,
    ErrNotificationNotFound = -11,
    ErrInternal = -12,
    ErrSnapshotNotFound = -13,
    ErrPolicyDenied = -14,
    ErrAmbiguousTarget = -15,
}

const _: () = assert!(AdResult::ErrPermDenied as i32 == -1);
const _: () = assert!(AdResult::ErrElementNotFound as i32 == -2);
const _: () = assert!(AdResult::ErrAppNotFound as i32 == -3);
const _: () = assert!(AdResult::ErrActionFailed as i32 == -4);
const _: () = assert!(AdResult::ErrActionNotSupported as i32 == -5);
const _: () = assert!(AdResult::ErrStaleRef as i32 == -6);
const _: () = assert!(AdResult::ErrWindowNotFound as i32 == -7);
const _: () = assert!(AdResult::ErrPlatformNotSupported as i32 == -8);
const _: () = assert!(AdResult::ErrTimeout as i32 == -9);
const _: () = assert!(AdResult::ErrInvalidArgs as i32 == -10);
const _: () = assert!(AdResult::ErrNotificationNotFound as i32 == -11);
const _: () = assert!(AdResult::ErrInternal as i32 == -12);
const _: () = assert!(AdResult::ErrSnapshotNotFound as i32 == -13);
const _: () = assert!(AdResult::ErrPolicyDenied as i32 == -14);
const _: () = assert!(AdResult::ErrAmbiguousTarget as i32 == -15);

enum MessageSource {
    Owned(CString),
    Static(&'static CStr),
}

impl MessageSource {
    fn as_ptr(&self) -> *const c_char {
        match self {
            MessageSource::Owned(cs) => cs.as_ptr(),
            MessageSource::Static(cs) => cs.as_ptr(),
        }
    }

    #[cfg(test)]
    fn to_owned_string(&self) -> String {
        match self {
            MessageSource::Owned(cs) => cs.to_string_lossy().into_owned(),
            MessageSource::Static(cs) => cs.to_string_lossy().into_owned(),
        }
    }
}

struct StoredError {
    code: AdResult,
    message: MessageSource,
    suggestion: Option<CString>,
    platform_detail: Option<CString>,
    details: Option<CString>,
}

static NUL_BYTE_FALLBACK: &CStr = c"(message contained null byte)";

thread_local! {
    static LAST_ERROR: RefCell<Option<StoredError>> = const { RefCell::new(None) };
}

fn error_code_to_result(code: &ErrorCode) -> AdResult {
    match code {
        ErrorCode::PermDenied => AdResult::ErrPermDenied,
        ErrorCode::ElementNotFound => AdResult::ErrElementNotFound,
        ErrorCode::AppNotFound => AdResult::ErrAppNotFound,
        ErrorCode::ActionFailed => AdResult::ErrActionFailed,
        ErrorCode::ActionNotSupported => AdResult::ErrActionNotSupported,
        ErrorCode::StaleRef => AdResult::ErrStaleRef,
        ErrorCode::AmbiguousTarget => AdResult::ErrAmbiguousTarget,
        ErrorCode::WindowNotFound => AdResult::ErrWindowNotFound,
        ErrorCode::PlatformNotSupported => AdResult::ErrPlatformNotSupported,
        ErrorCode::Timeout => AdResult::ErrTimeout,
        ErrorCode::InvalidArgs => AdResult::ErrInvalidArgs,
        ErrorCode::NotificationNotFound => AdResult::ErrNotificationNotFound,
        ErrorCode::Internal => AdResult::ErrInternal,
        ErrorCode::SnapshotNotFound => AdResult::ErrSnapshotNotFound,
        ErrorCode::PolicyDenied => AdResult::ErrPolicyDenied,
    }
}

pub(crate) fn set_last_error(err: &AdapterError) {
    let code = error_code_to_result(&err.code);
    let message = match CString::new(err.message.as_str()) {
        Ok(cs) => MessageSource::Owned(cs),
        Err(_) => MessageSource::Static(NUL_BYTE_FALLBACK),
    };
    let suggestion = err.suggestion.as_deref().and_then(|s| CString::new(s).ok());
    let platform_detail = err
        .platform_detail
        .as_deref()
        .and_then(|s| CString::new(s).ok());
    let details = err
        .details
        .as_ref()
        .and_then(|details| serde_json::to_string(details).ok())
        .and_then(|details| CString::new(details).ok());
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(StoredError {
            code,
            message,
            suggestion,
            platform_detail,
            details,
        });
    });
}

#[cfg(test)]
pub(crate) fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

/// Sets the last-error using a `'static CStr` message. Never allocates,
/// never panics — safe to call from a panic handler.
pub(crate) fn set_last_error_static(code: AdResult, message: &'static CStr) {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(StoredError {
            code,
            message: MessageSource::Static(message),
            suggestion: None,
            platform_detail: None,
            details: None,
        });
    });
}

pub(crate) fn last_error_code() -> AdResult {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|e| e.code)
            .unwrap_or(AdResult::Ok)
    })
}

/// Last-error lifetime — errno-style.
///
/// The pointer returned by `ad_last_error_message`,
/// `ad_last_error_suggestion`, and `ad_last_error_platform_detail`
/// remains valid across any number of subsequent **successful** FFI
/// calls on the same thread. Only the next FFI call that itself **fails**
/// (returns a non-`AD_RESULT_OK` code) invalidates the previous pointers.
///
/// Consumers can therefore read an error once, cache the pointer, and
/// keep reading it back across follow-up work that clears or re-fetches
/// state before handing control to the user.
///
/// This matches the POSIX `errno` / `strerror` contract and is scoped
/// per-thread via thread-local storage — Thread A's last-error never
/// leaks to Thread B.
/// Returns the `AdResult` code of the last error on the calling thread,
/// or `AD_RESULT_OK` if no error has been recorded.
#[unsafe(no_mangle)]
pub extern "C" fn ad_last_error_code() -> AdResult {
    crate::ffi_try::trap_panic(last_error_code)
}

/// Returns a borrowed C string describing the last error, or null if no
/// error has been recorded on the calling thread. The pointer remains
/// valid across any number of subsequent *successful* FFI calls; only
/// the next failing call overwrites it.
#[unsafe(no_mangle)]
pub extern "C" fn ad_last_error_message() -> *const c_char {
    crate::ffi_try::trap_panic_const_ptr(|| {
        LAST_ERROR.with(|cell| {
            cell.borrow()
                .as_ref()
                .map(|e| e.message.as_ptr())
                .unwrap_or(std::ptr::null())
        })
    })
}

/// Returns a borrowed C string with a human-readable suggestion for how
/// to recover from the last error, or null if the adapter didn't emit
/// one. Same lifetime rules as `ad_last_error_message`.
#[unsafe(no_mangle)]
pub extern "C" fn ad_last_error_suggestion() -> *const c_char {
    crate::ffi_try::trap_panic_const_ptr(|| {
        LAST_ERROR.with(|cell| {
            cell.borrow()
                .as_ref()
                .and_then(|e| e.suggestion.as_ref().map(|s| s.as_ptr()))
                .unwrap_or(std::ptr::null())
        })
    })
}

/// Returns a borrowed C string carrying a platform-specific diagnostic
/// for the last error (AX error codes, COM HRESULTs, AT-SPI messages,
/// etc.), or null if the adapter didn't supply one. Same lifetime rules
/// as `ad_last_error_message`.
#[unsafe(no_mangle)]
pub extern "C" fn ad_last_error_platform_detail() -> *const c_char {
    crate::ffi_try::trap_panic_const_ptr(|| {
        LAST_ERROR.with(|cell| {
            cell.borrow()
                .as_ref()
                .and_then(|e| e.platform_detail.as_ref().map(|s| s.as_ptr()))
                .unwrap_or(std::ptr::null())
        })
    })
}

/// Returns a borrowed JSON string carrying structured details for the last
/// error, or null if the adapter didn't supply any. Same lifetime rules as
/// `ad_last_error_message`.
#[unsafe(no_mangle)]
pub extern "C" fn ad_last_error_details() -> *const c_char {
    crate::ffi_try::trap_panic_const_ptr(|| {
        LAST_ERROR.with(|cell| {
            cell.borrow()
                .as_ref()
                .and_then(|e| e.details.as_ref().map(|s| s.as_ptr()))
                .unwrap_or(std::ptr::null())
        })
    })
}

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
