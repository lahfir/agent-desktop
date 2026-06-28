use crate::convert::string::{free_c_string, opt_string_to_c, string_to_c_lossy};
use crate::types::{AdSnapshotSurface, AdSurfaceInfo};
use agent_desktop_core::{
    adapter::SnapshotSurface,
    error::{AdapterError, ErrorCode},
    node::SurfaceInfo,
};
use std::os::raw::c_char;
use std::ptr;

pub(crate) fn surface_info_to_c(s: &SurfaceInfo) -> AdSurfaceInfo {
    AdSurfaceInfo {
        kind: string_to_c_lossy(&s.kind),
        title: opt_string_to_c(s.title.as_deref()),
        item_count: s.item_count.map(|c| c as i64).unwrap_or(-1),
    }
}

pub(crate) fn snapshot_surface_from_c(
    raw: i32,
    field: &str,
) -> Result<SnapshotSurface, AdapterError> {
    AdSnapshotSurface::from_c(raw)
        .map(|surface| match surface {
            AdSnapshotSurface::Window => SnapshotSurface::Window,
            AdSnapshotSurface::Focused => SnapshotSurface::Focused,
            AdSnapshotSurface::Menu => SnapshotSurface::Menu,
            AdSnapshotSurface::Menubar => SnapshotSurface::Menubar,
            AdSnapshotSurface::Sheet => SnapshotSurface::Sheet,
            AdSnapshotSurface::Popover => SnapshotSurface::Popover,
            AdSnapshotSurface::Alert => SnapshotSurface::Alert,
        })
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::InvalidArgs,
                format!("invalid {field} discriminant"),
            )
        })
}

pub(crate) unsafe fn free_surface_info_fields(s: &mut AdSurfaceInfo) {
    unsafe {
        free_c_string(s.kind as *mut c_char);
        free_c_string(s.title as *mut c_char);
        s.kind = ptr::null();
        s.title = ptr::null();
    }
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

    #[test]
    fn snapshot_surface_from_c_uses_shared_enum_validation() {
        assert_eq!(
            snapshot_surface_from_c(5, "source_surface").unwrap(),
            SnapshotSurface::Popover
        );

        let err = snapshot_surface_from_c(99, "source_surface").unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidArgs);
        assert_eq!(err.message, "invalid source_surface discriminant");
    }

    #[test]
    fn item_count_none_maps_to_sentinel_minus_one() {
        let s = SurfaceInfo {
            kind: "menu".into(),
            title: None,
            item_count: None,
        };
        let c = surface_info_to_c(&s);
        assert_eq!(c.item_count, -1);
        let mut c = c;
        unsafe { free_surface_info_fields(&mut c) };
    }

    #[test]
    fn item_count_some_zero_maps_to_zero_not_to_absent_sentinel() {
        let s = SurfaceInfo {
            kind: "popover".into(),
            title: None,
            item_count: Some(0),
        };
        let c = surface_info_to_c(&s);
        assert_eq!(
            c.item_count, 0,
            "Some(0) must map to 0, not to the -1 absent sentinel"
        );
        let mut c = c;
        unsafe { free_surface_info_fields(&mut c) };
    }

    #[test]
    fn title_some_maps_to_non_null_c_string_with_correct_value() {
        let s = SurfaceInfo {
            kind: "sheet".into(),
            title: Some("Save Panel".into()),
            item_count: None,
        };
        let c = surface_info_to_c(&s);
        assert!(!c.title.is_null());
        assert_eq!(
            unsafe { c_to_string(c.title) }.as_deref(),
            Some("Save Panel")
        );
        let mut c = c;
        unsafe { free_surface_info_fields(&mut c) };
    }

    #[test]
    fn snapshot_surface_from_c_maps_all_seven_variants_exactly() {
        let cases: [(i32, SnapshotSurface); 7] = [
            (0, SnapshotSurface::Window),
            (1, SnapshotSurface::Focused),
            (2, SnapshotSurface::Menu),
            (3, SnapshotSurface::Menubar),
            (4, SnapshotSurface::Sheet),
            (5, SnapshotSurface::Popover),
            (6, SnapshotSurface::Alert),
        ];
        for (raw, expected) in cases {
            assert_eq!(
                snapshot_surface_from_c(raw, "kind").unwrap(),
                expected,
                "raw discriminant {raw} must map to {expected:?}"
            );
        }
    }
}
