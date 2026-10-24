use super::*;

#[test]
fn only_gtk4_menu_frames_use_the_popup_origin() {
    for (name, version, role, expected) in [
        ("GTK", "4.14.5", Some(MENU), true),
        ("GTK", "3.24", Some(MENU), false),
        ("Qt", "6", Some(MENU), false),
        ("GTK", "4.14.5", Some(23), false),
        ("GTK", "4.14.5", None, false),
    ] {
        let toolkit = ToolkitId {
            name: Some(name.to_owned()),
            version: Some(version.to_owned()),
        };
        assert_eq!(uses_popup_origin(role, &toolkit), expected);
    }
}
