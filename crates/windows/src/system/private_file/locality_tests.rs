use super::Scratch;
use crate::system::private_file::WindowsPrivateFile;
use crate::system::private_file::locality::{
    AlignedRemoteProtocolInfo, FILE_REMOTE_PROTOCOL_INFO_SIZE, SurfaceLocality,
    assess_file_locality, basic_info_control_succeeds, classify_surface_locality,
    forced_control_failure::with_forced_control_failure, remote_protocol_probe,
};
use agent_desktop_core::PrivateFileOps;
use std::io::ErrorKind;
use std::os::windows::io::AsRawHandle;
use windows_sys::Win32::Foundation::ERROR_INVALID_PARAMETER;
use windows_sys::Win32::Storage::FileSystem::GetFileInformationByHandleEx;

const OUT_OF_RANGE_INFO_CLASS: i32 = 55;

#[test]
fn the_classifier_requires_a_succeeding_control_call_before_87_means_local() {
    assert_eq!(
        classify_surface_locality(true, Err(ERROR_INVALID_PARAMETER)),
        SurfaceLocality::Local
    );
    assert_eq!(
        classify_surface_locality(true, Ok(())),
        SurfaceLocality::Remote
    );
    assert_eq!(
        classify_surface_locality(false, Err(ERROR_INVALID_PARAMETER)),
        SurfaceLocality::Unknown
    );
    assert_eq!(
        classify_surface_locality(true, Err(5)),
        SurfaceLocality::Unknown
    );
}

#[test]
fn class_13_fails_87_on_a_local_ntfs_file_while_the_control_class_succeeds_on_the_same_handle() {
    let scratch = Scratch::new("locality-local");
    let path = scratch.path().join("target.bin");
    std::fs::write(&path, b"local bytes").unwrap();
    let file = std::fs::File::open(&path).unwrap();

    assert!(
        basic_info_control_succeeds(file.as_raw_handle()),
        "the FileBasicInfo control call must succeed on a local handle"
    );
    assert_eq!(
        remote_protocol_probe(file.as_raw_handle()),
        Err(ERROR_INVALID_PARAMETER),
        "class 13 must fail with 87 on local NTFS"
    );
    assert_eq!(assess_file_locality(&file), SurfaceLocality::Local);
}

#[test]
fn an_out_of_range_class_returns_the_same_87_so_87_alone_proves_nothing() {
    let scratch = Scratch::new("locality-out-of-range");
    let path = scratch.path().join("target.bin");
    std::fs::write(&path, b"local bytes").unwrap();
    let file = std::fs::File::open(&path).unwrap();

    let mut information = AlignedRemoteProtocolInfo::zeroed();
    let succeeded = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            OUT_OF_RANGE_INFO_CLASS,
            std::ptr::from_mut(&mut information).cast(),
            FILE_REMOTE_PROTOCOL_INFO_SIZE as u32,
        )
    };

    assert_eq!(succeeded, 0, "an out-of-range class must fail");
    assert_eq!(
        std::io::Error::last_os_error().raw_os_error(),
        Some(ERROR_INVALID_PARAMETER as i32),
        "the out-of-range failure must be the same 87 the locality probe sees"
    );
}

#[test]
fn a_forced_control_failure_yields_unknown_and_the_private_write_is_refused() {
    let scratch = Scratch::new("locality-unknown");
    let probe_path = scratch.path().join("probe.bin");
    std::fs::write(&probe_path, b"probe").unwrap();
    let probe_file = std::fs::File::open(&probe_path).unwrap();
    let refused_artifact = scratch.path().join("gated-parent").join("artifact.json");

    with_forced_control_failure(|| {
        assert_eq!(assess_file_locality(&probe_file), SurfaceLocality::Unknown);
        let refused = WindowsPrivateFile::new()
            .write_atomic(&refused_artifact, b"secret")
            .unwrap_err();
        assert_eq!(refused.kind(), ErrorKind::PermissionDenied);
        assert!(
            refused
                .to_string()
                .contains("locality could not be determined"),
            "the refusal must name the unknown locality: {refused}"
        );
    });

    assert!(
        !refused_artifact.exists(),
        "no artifact may exist after the refused write"
    );
    WindowsPrivateFile::new()
        .write_atomic(
            &scratch.path().join("ungated-parent").join("artifact.json"),
            b"local again",
        )
        .expect("writes must work again once the control call is restored");
}
