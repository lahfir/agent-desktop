#![cfg(unix)]

use super::*;
use std::ffi::CString;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::fs::PermissionsExt;

fn directory(label: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "agent-desktop-private-{label}-{}",
        crate::refs::new_snapshot_id()
    ));
    std::fs::create_dir_all(&path).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[test]
fn private_open_sets_nonblocking_and_close_on_exec() {
    let directory = directory("flags");
    let path = directory.join("lock");
    let file = open_private_lock(&path, true).unwrap();

    let descriptor_flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
    let status_flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFL) };
    assert_ne!(descriptor_flags & libc::FD_CLOEXEC, 0);
    assert_ne!(status_flags & libc::O_NONBLOCK, 0);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn private_open_rejects_symlink_fifo_device_and_hardlink() {
    let directory = directory("special");
    let target = directory.join("target");
    let target_file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&target)
        .unwrap();
    drop(target_file);
    let symlink = directory.join("symlink");
    std::os::unix::fs::symlink(&target, &symlink).unwrap();
    assert!(open_private_lock(&symlink, false).is_err());

    let fifo = directory.join("fifo");
    let fifo_path = CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) }, 0);
    let started = std::time::Instant::now();
    assert!(open_private_lock(&fifo, false).is_err());
    assert!(started.elapsed() < std::time::Duration::from_secs(1));

    let hardlink = directory.join("hardlink");
    std::fs::hard_link(&target, &hardlink).unwrap();
    assert!(open_private_lock(&hardlink, false).is_err());
    assert!(open_private_lock(Path::new("/dev/null"), false).is_err());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn private_read_enforces_the_bound_before_allocating() {
    let directory = directory("bound");
    let path = directory.join("data");
    write_atomic(&path, b"12345").unwrap();

    let error = read_private_bounded(&path, 4).unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn private_writes_and_locks_reject_group_accessible_parent() {
    let directory = directory("hostile-parent");
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o770)).unwrap();
    let path = directory.join("data");

    assert_eq!(
        write_atomic(&path, b"private").unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert_eq!(
        open_private_lock(&path, true).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );

    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn private_write_rejects_an_intermediate_directory_symlink() {
    let directory = directory("intermediate-symlink");
    let outside = directory.join("outside");
    let nested = outside.join("nested");
    std::fs::create_dir_all(&nested).unwrap();
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::set_permissions(&nested, std::fs::Permissions::from_mode(0o700)).unwrap();
    let link = directory.join("redirect");
    std::os::unix::fs::symlink(&outside, &link).unwrap();

    let error = write_atomic(&link.join("nested/artifact"), b"private").unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    assert!(!nested.join("artifact").exists());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn user_write_allows_the_system_temporary_directory() {
    let path = Path::new("/tmp").join(format!(
        "agent-desktop-user-output-{}",
        crate::refs::new_snapshot_id()
    ));

    write_user_atomic(&path, b"private").unwrap();

    assert_eq!(std::fs::read(&path).unwrap(), b"private");
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::remove_file(path).unwrap();
}
