#![allow(dead_code)]

use crate::types::*;
use agent_desktop_core::node::{AppInfo, Rect, SurfaceInfo, WindowInfo};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

/// Converts a Rust `String` to a heap-allocated C string.
/// Caller must free with `ad_free_string`. Returns null on interior NUL.
pub(crate) fn string_to_c(s: &str) -> *mut c_char {
    match CString::new(s) {
        Ok(cs) => cs.into_raw(),
        Err(_) => ptr::null_mut(),
    }
}

/// Converts an `Option<String>` to a C string pointer (null if None).
pub(crate) fn opt_string_to_c(s: Option<&str>) -> *mut c_char {
    match s {
        Some(s) => string_to_c(s),
        None => ptr::null_mut(),
    }
}

/// Reclaims a C string previously created by `string_to_c`.
/// No-op if null.
pub(crate) unsafe fn free_c_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        drop(CString::from_raw(ptr));
    }
}

/// Reads a borrowed `&str` from a C string pointer.
/// Returns None if null or invalid UTF-8.
pub(crate) unsafe fn c_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    CStr::from_ptr(ptr).to_str().ok()
}

/// Converts a Rust `Rect` to `AdRect`.
pub(crate) fn rect_to_c(r: &Rect) -> AdRect {
    AdRect {
        x: r.x,
        y: r.y,
        width: r.width,
        height: r.height,
    }
}

/// Converts a Rust `WindowInfo` to `AdWindowInfo`.
/// All strings are heap-allocated and must be freed with `ad_free_window`.
pub(crate) fn window_info_to_c(w: &WindowInfo) -> AdWindowInfo {
    let (bounds, has_bounds) = match &w.bounds {
        Some(r) => (rect_to_c(r), true),
        None => (
            AdRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            false,
        ),
    };
    AdWindowInfo {
        id: string_to_c(&w.id),
        title: string_to_c(&w.title),
        app_name: string_to_c(&w.app),
        pid: w.pid,
        bounds,
        has_bounds,
        is_focused: w.is_focused,
    }
}

/// Converts a Rust `AppInfo` to `AdAppInfo`.
pub(crate) fn app_info_to_c(a: &AppInfo) -> AdAppInfo {
    AdAppInfo {
        name: string_to_c(&a.name),
        pid: a.pid,
        bundle_id: opt_string_to_c(a.bundle_id.as_deref()),
    }
}

/// Converts a Rust `SurfaceInfo` to `AdSurfaceInfo`.
pub(crate) fn surface_info_to_c(s: &SurfaceInfo) -> AdSurfaceInfo {
    AdSurfaceInfo {
        kind: string_to_c(&s.kind),
        title: opt_string_to_c(s.title.as_deref()),
        item_count: s.item_count.map(|c| c as i64).unwrap_or(-1),
    }
}

/// Frees all strings inside an `AdWindowInfo`. Does NOT free the struct pointer.
pub(crate) unsafe fn free_window_info_fields(w: &mut AdWindowInfo) {
    free_c_string(w.id as *mut c_char);
    free_c_string(w.title as *mut c_char);
    free_c_string(w.app_name as *mut c_char);
    w.id = ptr::null();
    w.title = ptr::null();
    w.app_name = ptr::null();
}

/// Frees all strings inside an `AdAppInfo`.
pub(crate) unsafe fn free_app_info_fields(a: &mut AdAppInfo) {
    free_c_string(a.name as *mut c_char);
    free_c_string(a.bundle_id as *mut c_char);
    a.name = ptr::null();
    a.bundle_id = ptr::null();
}

/// Frees all strings inside an `AdSurfaceInfo`.
pub(crate) unsafe fn free_surface_info_fields(s: &mut AdSurfaceInfo) {
    free_c_string(s.kind as *mut c_char);
    free_c_string(s.title as *mut c_char);
    s.kind = ptr::null();
    s.title = ptr::null();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_roundtrip() {
        let c = string_to_c("hello");
        assert!(!c.is_null());
        let back = unsafe { c_to_str(c) };
        assert_eq!(back, Some("hello"));
        unsafe { free_c_string(c) };
    }

    #[test]
    fn test_null_string() {
        let c = opt_string_to_c(None);
        assert!(c.is_null());
        let back = unsafe { c_to_str(c) };
        assert_eq!(back, None);
    }

    #[test]
    fn test_free_null_is_noop() {
        unsafe { free_c_string(ptr::null_mut()) };
    }

    #[test]
    fn test_window_info_roundtrip() {
        let w = WindowInfo {
            id: "w-123".into(),
            title: "Documents".into(),
            app: "Finder".into(),
            pid: 42,
            bounds: Some(Rect {
                x: 10.0,
                y: 20.0,
                width: 800.0,
                height: 600.0,
            }),
            is_focused: true,
        };
        let c = window_info_to_c(&w);
        assert_eq!(unsafe { c_to_str(c.id) }, Some("w-123"));
        assert_eq!(unsafe { c_to_str(c.title) }, Some("Documents"));
        assert_eq!(unsafe { c_to_str(c.app_name) }, Some("Finder"));
        assert_eq!(c.pid, 42);
        assert!(c.has_bounds);
        assert_eq!(c.bounds.x, 10.0);
        assert!(c.is_focused);
        let mut c = c;
        unsafe { free_window_info_fields(&mut c) };
    }

    #[test]
    fn test_app_info_roundtrip() {
        let a = AppInfo {
            name: "Finder".into(),
            pid: 42,
            bundle_id: Some("com.apple.finder".into()),
        };
        let c = app_info_to_c(&a);
        assert_eq!(unsafe { c_to_str(c.name) }, Some("Finder"));
        assert_eq!(unsafe { c_to_str(c.bundle_id) }, Some("com.apple.finder"));
        let mut c = c;
        unsafe { free_app_info_fields(&mut c) };
    }

    #[test]
    fn test_surface_info_no_title() {
        let s = SurfaceInfo {
            kind: "menu".into(),
            title: None,
            item_count: Some(3),
        };
        let c = surface_info_to_c(&s);
        assert_eq!(unsafe { c_to_str(c.kind) }, Some("menu"));
        assert!(c.title.is_null());
        assert_eq!(c.item_count, 3);
        let mut c = c;
        unsafe { free_surface_info_fields(&mut c) };
    }
}
