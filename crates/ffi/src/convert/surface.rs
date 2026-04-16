use crate::convert::string::{free_c_string, opt_string_to_c, string_to_c_lossy};
use crate::types::AdSurfaceInfo;
use agent_desktop_core::node::SurfaceInfo;
use std::os::raw::c_char;
use std::ptr;

pub(crate) fn surface_info_to_c(s: &SurfaceInfo) -> AdSurfaceInfo {
    AdSurfaceInfo {
        kind: string_to_c_lossy(&s.kind),
        title: opt_string_to_c(s.title.as_deref()),
        item_count: s.item_count.map(|c| c as i64).unwrap_or(-1),
    }
}

pub(crate) unsafe fn free_surface_info_fields(s: &mut AdSurfaceInfo) {
    free_c_string(s.kind as *mut c_char);
    free_c_string(s.title as *mut c_char);
    s.kind = ptr::null();
    s.title = ptr::null();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::convert::string::c_to_string;

    #[test]
    fn test_surface_info_no_title() {
        let s = SurfaceInfo {
            kind: "menu".into(),
            title: None,
            item_count: Some(3),
        };
        let c = surface_info_to_c(&s);
        assert_eq!(unsafe { c_to_string(c.kind) }.as_deref(), Some("menu"));
        assert!(c.title.is_null());
        assert_eq!(c.item_count, 3);
        let mut c = c;
        unsafe { free_surface_info_fields(&mut c) };
    }
}
