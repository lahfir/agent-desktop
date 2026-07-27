use super::Scratch;
use crate::system::private_file::WindowsPrivateFile;
use crate::system::private_file::replace::{
    replace_file_call, replace_style_failure_detail, to_wide_null,
};
use agent_desktop_core::PrivateFileOps;
use std::fs::{File, OpenOptions};
use std::io::Read;
use std::os::windows::fs::OpenOptionsExt;
use std::path::Path;
use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, MOVEFILE_REPLACE_EXISTING, MoveFileExW,
};

const SHARE_NONE: u32 = 0;
const SHARE_ALL: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;

fn open_reader_with_share(path: &Path, share: u32) -> File {
    OpenOptions::new()
        .read(true)
        .share_mode(share)
        .open(path)
        .expect("the holder handle must open")
}

fn move_file_call(source: &Path, destination: &Path) -> std::io::Result<()> {
    let source_wide = to_wide_null(source)?;
    let destination_wide = to_wide_null(destination)?;
    let succeeded = unsafe {
        MoveFileExW(
            source_wide.as_ptr(),
            destination_wide.as_ptr(),
            MOVEFILE_REPLACE_EXISTING,
        )
    };
    if succeeded != 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

fn seeded_pair(scratch: &Scratch) -> (std::path::PathBuf, std::path::PathBuf) {
    let destination = scratch.path().join("destination.bin");
    let replacement = scratch.path().join("replacement.bin");
    std::fs::write(&destination, b"old bytes").unwrap();
    std::fs::write(&replacement, b"new bytes").unwrap();
    (destination, replacement)
}

#[test]
fn replace_succeeds_over_a_destination_held_with_share_delete_and_the_held_handle_reads_old_bytes()
{
    let scratch = Scratch::new("replace-share-all");
    let (destination, replacement) = seeded_pair(&scratch);
    let held = open_reader_with_share(&destination, SHARE_ALL);

    replace_file_call(&destination, &replacement)
        .expect("ReplaceFileW must succeed over a share-delete holder");

    let mut held_view = String::new();
    (&held)
        .read_to_string(&mut held_view)
        .expect("the held handle must stay readable");
    assert_eq!(
        held_view, "old bytes",
        "the held handle must still read the old bytes"
    );
    assert_eq!(
        std::fs::read(&destination).unwrap(),
        b"new bytes",
        "a fresh open must observe the replacement bytes"
    );
}

#[test]
fn replace_succeeds_over_a_destination_held_with_share_delete_only() {
    let scratch = Scratch::new("replace-share-delete-only");
    let (destination, replacement) = seeded_pair(&scratch);
    let held = open_reader_with_share(&destination, FILE_SHARE_DELETE);

    replace_file_call(&destination, &replacement)
        .expect("ReplaceFileW must succeed over a delete-only-share holder");

    let mut held_view = String::new();
    (&held).read_to_string(&mut held_view).unwrap();
    assert_eq!(held_view, "old bytes");
    drop(held);
    assert_eq!(std::fs::read(&destination).unwrap(), b"new bytes");
}

#[test]
fn replace_fails_32_over_a_destination_held_without_share_delete() {
    for share in [
        SHARE_NONE,
        FILE_SHARE_READ,
        FILE_SHARE_READ | FILE_SHARE_WRITE,
    ] {
        let scratch = Scratch::new("replace-no-share-delete");
        let (destination, replacement) = seeded_pair(&scratch);
        let held = open_reader_with_share(&destination, share);

        let refused = replace_file_call(&destination, &replacement).unwrap_err();

        assert_eq!(
            refused.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION as i32),
            "share mode {share:#x} must fail with ERROR_SHARING_VIOLATION"
        );
        drop(held);
        assert_eq!(
            std::fs::read(&destination).unwrap(),
            b"old bytes",
            "the destination must be untouched after the refused replace"
        );
    }
}

#[test]
fn replace_fails_32_over_an_open_source_even_with_share_delete() {
    for share in [FILE_SHARE_DELETE, SHARE_ALL] {
        let scratch = Scratch::new("replace-open-source");
        let (destination, replacement) = seeded_pair(&scratch);
        let held = open_reader_with_share(&replacement, share);

        let refused = replace_file_call(&destination, &replacement).unwrap_err();

        assert_eq!(
            refused.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION as i32),
            "an open source must fail with ERROR_SHARING_VIOLATION even at share {share:#x}"
        );
        drop(held);
        assert_eq!(std::fs::read(&destination).unwrap(), b"old bytes");
    }
}

#[test]
fn move_file_ex_fails_5_not_32_over_an_open_target_even_at_full_share() {
    let scratch = Scratch::new("move-open-target");
    let (destination, replacement) = seeded_pair(&scratch);
    let held = open_reader_with_share(&destination, SHARE_ALL);

    let refused = move_file_call(&replacement, &destination).unwrap_err();

    assert_ne!(
        refused.raw_os_error(),
        Some(ERROR_SHARING_VIOLATION as i32),
        "move-style ops do not report the destination-side failure as a sharing violation"
    );
    assert_eq!(
        refused.raw_os_error(),
        Some(ERROR_ACCESS_DENIED as i32),
        "MoveFileExW over an open target must fail with ERROR_ACCESS_DENIED"
    );
    drop(held);
    assert_eq!(std::fs::read(&destination).unwrap(), b"old bytes");
}

#[test]
fn error_5_and_32_classify_to_opposite_sides_for_move_and_replace_style_ops() {
    assert!(
        replace_style_failure_detail(ERROR_SHARING_VIOLATION)
            .contains("destination-side signature for replace-style")
    );
    assert!(replace_style_failure_detail(ERROR_ACCESS_DENIED).contains("move-style"));
}

#[test]
fn write_atomic_replaces_a_destination_held_open_by_a_wide_share_reader() {
    let scratch = Scratch::new("write-over-reader");
    let destination = scratch.path().join("refmap.json");
    let ops = WindowsPrivateFile::new();
    ops.write_atomic(&destination, b"{\"v\":1}").unwrap();
    let held = open_reader_with_share(&destination, SHARE_ALL);

    ops.write_atomic(&destination, b"{\"v\":2}").unwrap();

    let mut held_view = String::new();
    (&held).read_to_string(&mut held_view).unwrap();
    assert_eq!(held_view, "{\"v\":1}");
    assert_eq!(
        ops.read_private_bounded(&destination, 64).unwrap(),
        b"{\"v\":2}"
    );
}

#[test]
fn the_first_write_into_a_parent_reclaims_a_dead_processes_lease_directory() {
    let scratch = Scratch::new("sweep-stale");
    let stale = scratch.path().join(".agent-desktop-tmp-p4294967295");
    std::fs::create_dir(&stale).unwrap();
    std::fs::write(stale.join(".orphan.tmp"), b"leftover").unwrap();
    let ops = WindowsPrivateFile::new();

    ops.write_atomic(&scratch.path().join("artifact.json"), b"fresh")
        .unwrap();

    assert!(
        !stale.exists(),
        "a lease directory with no live owner must be reclaimed"
    );
    let own_lease = scratch
        .path()
        .join(format!(".agent-desktop-tmp-p{}", std::process::id()));
    assert!(own_lease.is_dir(), "this process's lease must exist");
}

#[test]
fn a_lease_directory_whose_owner_still_lives_survives_the_sweep() {
    let scratch = Scratch::new("sweep-live");
    let foreign = scratch.path().join(".agent-desktop-tmp-p1");
    std::fs::create_dir(&foreign).unwrap();
    let liveness_handle = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_BACKUP_SEMANTICS)
        .open(&foreign)
        .unwrap();
    let ops = WindowsPrivateFile::new();

    ops.write_atomic(&scratch.path().join("artifact.json"), b"fresh")
        .unwrap();

    assert!(
        foreign.is_dir(),
        "a lease directory whose liveness handle is held must not be reclaimed"
    );
    drop(liveness_handle);
}

#[test]
fn temporaries_are_confined_to_the_lease_directory_and_consumed_on_success() {
    let scratch = Scratch::new("temp-confinement");
    let ops = WindowsPrivateFile::new();

    ops.write_atomic(&scratch.path().join("artifact.json"), b"payload")
        .unwrap();

    let own_lease_name = format!(".agent-desktop-tmp-p{}", std::process::id());
    for entry in std::fs::read_dir(scratch.path()).unwrap().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        assert!(
            name == "artifact.json" || name == own_lease_name,
            "unexpected sibling {name}: temporaries must live inside the lease directory"
        );
    }
    let leftovers: Vec<_> = std::fs::read_dir(scratch.path().join(&own_lease_name))
        .unwrap()
        .flatten()
        .collect();
    assert!(
        leftovers.is_empty(),
        "a successful write must consume its temporary"
    );
}
