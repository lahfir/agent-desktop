use super::*;

#[test]
fn bundle_root_is_derived_from_the_conventional_executable_layout() {
    let executable = Path::new("/Applications/Fixture.app/Contents/MacOS/Fixture");

    assert_eq!(
        bundle_root_from_executable(executable),
        Some(PathBuf::from("/Applications/Fixture.app"))
    );
}

#[test]
fn bundle_root_is_none_for_a_bare_executable_outside_any_bundle() {
    assert_eq!(
        bundle_root_from_executable(Path::new("/usr/bin/fixture")),
        None
    );
}

#[test]
fn bundle_root_is_none_when_the_macos_directory_is_missing() {
    let executable = Path::new("/Applications/Fixture.app/Contents/Fixture");

    assert_eq!(bundle_root_from_executable(executable), None);
}

#[test]
fn bundle_root_is_none_when_the_containing_directory_is_not_an_app_bundle() {
    let executable = Path::new("/opt/Fixture/Contents/MacOS/Fixture");

    assert_eq!(bundle_root_from_executable(executable), None);
}

#[test]
fn electron_and_cef_framework_names_are_recognized_exactly() {
    assert!(is_chromium_framework_marker("Electron Framework.framework"));
    assert!(is_chromium_framework_marker(
        "Chromium Embedded Framework.framework"
    ));
}

#[test]
fn unrelated_or_partial_framework_names_are_not_recognized() {
    assert!(!is_chromium_framework_marker("Sparkle.framework"));
    assert!(!is_chromium_framework_marker("Electron Framework"));
    assert!(!is_chromium_framework_marker(
        "electron framework.framework"
    ));
}
