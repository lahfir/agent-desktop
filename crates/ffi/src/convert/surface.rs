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

pub(crate) fn snapshot_surface_to_core(surface: AdSnapshotSurface) -> SnapshotSurface {
    match surface {
        AdSnapshotSurface::Window => SnapshotSurface::Window,
        AdSnapshotSurface::Focused => SnapshotSurface::Focused,
        AdSnapshotSurface::Menu => SnapshotSurface::Menu,
        AdSnapshotSurface::Menubar => SnapshotSurface::Menubar,
        AdSnapshotSurface::Sheet => SnapshotSurface::Sheet,
        AdSnapshotSurface::Popover => SnapshotSurface::Popover,
        AdSnapshotSurface::Alert => SnapshotSurface::Alert,
    }
}

pub(crate) fn snapshot_surface_from_c(
    raw: i32,
    field: &str,
) -> Result<SnapshotSurface, AdapterError> {
    AdSnapshotSurface::from_c(raw)
        .map(snapshot_surface_to_core)
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
}
