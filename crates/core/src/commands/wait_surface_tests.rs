use super::SurfaceWait;

#[test]
fn no_flags_selects_no_surface_wait() {
    assert_eq!(SurfaceWait::from_flags(false, false, false).unwrap(), None);
}

#[test]
fn each_flag_maps_to_its_variant() {
    assert_eq!(
        SurfaceWait::from_flags(true, false, false).unwrap(),
        Some(SurfaceWait::Menu)
    );
    assert_eq!(
        SurfaceWait::from_flags(false, true, false).unwrap(),
        Some(SurfaceWait::MenuClosed)
    );
    assert_eq!(
        SurfaceWait::from_flags(false, false, true).unwrap(),
        Some(SurfaceWait::Notification)
    );
}

#[test]
fn conflicting_flags_report_exactly_one_mode_error() {
    let err = SurfaceWait::from_flags(true, false, true).unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
    assert_eq!(err.to_string(), "wait accepts exactly one mode");
    assert!(err.suggestion().is_some());
}

#[test]
fn menu_and_menu_closed_together_are_rejected() {
    let err = SurfaceWait::from_flags(true, true, false).unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}
