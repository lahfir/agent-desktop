use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

pub(crate) fn string_to_c(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Lossy variant for **mandatory** C-string fields. Replaces any interior
/// NUL byte with U+FFFD (3-byte UTF-8) before `CString::new`, so the
/// result is never null on NUL-containing input. Use for fields the
/// ABI contract forbids from being null (role, app_name, kind, ...).
/// Optional fields should keep using `opt_string_to_c`.
pub(crate) fn string_to_c_lossy(s: &str) -> *mut c_char {
    if !s.bytes().any(|b| b == 0) {
        return string_to_c(s);
    }
    let cleaned: String = s
        .chars()
        .map(|c| if c == '\0' { '\u{FFFD}' } else { c })
        .collect();
    match CString::new(cleaned) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

pub(crate) fn opt_string_to_c(s: Option<&str>) -> *mut c_char {
    match s {
        Some(s) => string_to_c(s),
        None => ptr::null_mut(),
    }
}

pub(crate) unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

pub(crate) unsafe fn c_to_string(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok().map(str::to_owned)
}

/// Tri-state decode of a foreign C string used for optional filter
/// fields where conflating null with invalid UTF-8 would silently widen
/// an operation (e.g. treat "bad bytes" as "no filter").
///
/// - `Ok(None)` — pointer is null. Caller should treat as "filter
///   absent".
/// - `Ok(Some(s))` — pointer is non-null and decodes as valid UTF-8.
/// - `Err(())` — pointer is non-null but the bytes are not UTF-8.
///   Caller should surface `AD_RESULT_ERR_INVALID_ARGS` instead of
///   treating this as missing.
///
/// # Safety
/// `ptr` must be null or a NUL-terminated C string.
pub(crate) unsafe fn try_c_to_string(ptr: *const c_char) -> Result<Option<String>, ()> {
    if ptr.is_null() {
        return Ok(None);
    }
    match CStr::from_ptr(ptr).to_str() {
        Ok(s) => Ok(Some(s.to_owned())),
        Err(_) => Err(()),
    }
}

/// Decode an optional filter string, short-circuiting the enclosing
/// `AdResult`-returning fn with `AD_RESULT_ERR_INVALID_ARGS` (and a
/// tailored last-error diagnostic) when the pointer is non-null but
/// the bytes are not UTF-8. Null → `None` (treated as "no filter").
macro_rules! decode_optional_filter {
    ($ptr:expr, $label:expr) => {{
        match $crate::convert::string::try_c_to_string($ptr) {
            Ok(value) => value,
            Err(()) => {
                $crate::error::set_last_error(&agent_desktop_core::error::AdapterError::new(
                    agent_desktop_core::error::ErrorCode::InvalidArgs,
                    concat!($label, " is not valid UTF-8"),
                ));
                return $crate::error::AdResult::ErrInvalidArgs;
            }
        }
    }};
}

pub(crate) use decode_optional_filter;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_roundtrip() {
        let c = string_to_c("hello");
        assert!(!c.is_null());
        let back = unsafe { c_to_string(c) };
        assert_eq!(back.as_deref(), Some("hello"));
        unsafe { free_c_string(c) };
    }

    #[test]
    fn test_null_string() {
        let c = opt_string_to_c(None);
        assert!(c.is_null());
        let back = unsafe { c_to_string(c) };
        assert_eq!(back, None);
    }

    #[test]
    fn test_free_null_is_noop() {
        unsafe { free_c_string(ptr::null_mut()) };
    }

    #[test]
    fn test_lossy_no_nul_same_as_string_to_c() {
        let c = string_to_c_lossy("hello");
        assert!(!c.is_null());
        let back = unsafe { c_to_string(c) };
        assert_eq!(back.as_deref(), Some("hello"));
        unsafe { free_c_string(c) };
    }

    #[test]
    fn test_lossy_replaces_interior_nul() {
        let c = string_to_c_lossy("foo\0\0bar");
        assert!(!c.is_null());
        let back = unsafe { c_to_string(c) }.unwrap();
        assert_eq!(back, "foo\u{FFFD}\u{FFFD}bar");
        unsafe { free_c_string(c) };
    }

    #[test]
    fn test_lossy_handles_all_nul_input() {
        let c = string_to_c_lossy("\0\0\0");
        assert!(!c.is_null());
        let back = unsafe { c_to_string(c) }.unwrap();
        assert_eq!(back, "\u{FFFD}\u{FFFD}\u{FFFD}");
        unsafe { free_c_string(c) };
    }

    #[test]
    fn try_c_to_string_null_is_ok_none() {
        let result = unsafe { try_c_to_string(ptr::null()) };
        assert!(matches!(result, Ok(None)));
    }

    #[test]
    fn try_c_to_string_valid_utf8_is_some() {
        let c = string_to_c("agent");
        let result = unsafe { try_c_to_string(c) };
        assert!(matches!(result, Ok(Some(ref s)) if s == "agent"));
        unsafe { free_c_string(c) };
    }

    #[test]
    fn try_c_to_string_invalid_utf8_is_err() {
        let bad: [u8; 3] = [0xC3, 0xFF, 0x00];
        let result = unsafe { try_c_to_string(bad.as_ptr() as *const c_char) };
        assert!(matches!(result, Err(())));
    }
}
