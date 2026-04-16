use crate::convert::rect::rect_to_c;
use crate::convert::string::{free_c_string, string_to_c_lossy};
use crate::types::{AdRect, AdWindowInfo};
use agent_desktop_core::node::WindowInfo;
use std::os::raw::c_char;
use std::ptr;

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
        id: string_to_c_lossy(&w.id),
        title: string_to_c_lossy(&w.title),
        app_name: string_to_c_lossy(&w.app),
        pid: w.pid,
        bounds,
        has_bounds,
        is_focused: w.is_focused,
    }
}

pub(crate) unsafe fn free_window_info_fields(w: &mut AdWindowInfo) {
    free_c_string(w.id as *mut c_char);
    free_c_string(w.title as *mut c_char);
    free_c_string(w.app_name as *mut c_char);
    w.id = ptr::null();
    w.title = ptr::null();
    w.app_name = ptr::null();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::string::c_to_string;
    use agent_desktop_core::node::Rect;

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
        assert_eq!(unsafe { c_to_string(c.id) }.as_deref(), Some("w-123"));
        assert_eq!(
            unsafe { c_to_string(c.title) }.as_deref(),
            Some("Documents")
        );
        assert_eq!(
            unsafe { c_to_string(c.app_name) }.as_deref(),
            Some("Finder")
        );
        assert_eq!(c.pid, 42);
        assert!(c.has_bounds);
        assert_eq!(c.bounds.x, 10.0);
        assert!(c.is_focused);
        let mut c = c;
        unsafe { free_window_info_fields(&mut c) };
    }
}
