use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

pub(crate) fn string_to_c(s: &str) -> *mut c_char {
    match CString::new(s) {
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
}
