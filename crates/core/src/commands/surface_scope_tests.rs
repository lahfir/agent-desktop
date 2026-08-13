use super::reject_root_with_surface;
use crate::SnapshotSurface;

#[test]
fn a_ref_root_may_keep_the_default_window_surface() {
    assert!(reject_root_with_surface("find", Some("@s1:e2"), SnapshotSurface::Window).is_ok());
}

#[test]
fn a_ref_root_rejects_an_explicit_second_surface() {
    let error = reject_root_with_surface("find", Some("@s1:e2"), SnapshotSurface::Menubar)
        .expect_err("a ref already carries its surface");
    assert!(error.to_string().contains("--surface"));
}

#[test]
fn a_surface_without_a_root_is_allowed() {
    assert!(reject_root_with_surface("find", None, SnapshotSurface::Menubar).is_ok());
}
