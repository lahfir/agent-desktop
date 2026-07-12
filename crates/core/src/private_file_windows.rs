use std::fs::File;
use std::mem::size_of;
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr::null;
use windows_sys::Win32::Foundation::{
    ERROR_ALREADY_EXISTS, ERROR_INVALID_PARAMETER, GENERIC_READ, GENERIC_WRITE, GetLastError,
    HANDLE,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CREATE_NEW, CreateDirectoryW, DELETE, FILE_APPEND_DATA,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_REMOTE_PROTOCOL_INFO,
    FILE_SHARE_READ, FILE_SHARE_WRITE, FileRemoteProtocolInfo, GetFileInformationByHandle,
    GetFileInformationByHandleEx, OPEN_ALWAYS, OPEN_EXISTING, READ_CONTROL,
};

#[path = "private_file_windows_guard.rs"]
mod guard;
#[path = "private_file_windows_open.rs"]
mod open;
#[path = "private_file_windows_path.rs"]
mod path;
#[path = "private_file_windows_rename.rs"]
mod rename;
#[path = "private_file_windows_security.rs"]
mod security;

use open::FileOpen;
use security::PrivateSecurity;

const LEAF_SHARING: u32 = FILE_SHARE_READ | FILE_SHARE_WRITE;
const GUARD_SHARING: u32 = FILE_SHARE_READ;
const TEMP_ACCESS: u32 = GENERIC_WRITE | READ_CONTROL | DELETE;

pub(super) fn open_lock(path: &Path, create: bool) -> std::io::Result<File> {
    let creation = if create { OPEN_ALWAYS } else { OPEN_EXISTING };
    open_private(
        path,
        GENERIC_READ | GENERIC_WRITE | READ_CONTROL,
        creation,
        true,
    )
}

pub(super) fn open_append(path: &Path) -> std::io::Result<File> {
    open_private(path, FILE_APPEND_DATA | READ_CONTROL, OPEN_ALWAYS, false)
}

pub(super) fn open_read(path: &Path) -> std::io::Result<File> {
    open_private(path, GENERIC_READ | READ_CONTROL, OPEN_EXISTING, false)
}

pub(super) fn open_regular_read(path: &Path) -> std::io::Result<File> {
    let path = path::normalized(path)?;
    path::validate_file_name(&path)?;
    with_leaf_parent(&path, false, None, || {
        FileOpen::leaf(&path, GENERIC_READ, OPEN_EXISTING, null()).execute()
    })
}

pub(super) fn create_new(path: &Path) -> std::io::Result<File> {
    open_private(path, TEMP_ACCESS, CREATE_NEW, false)
}

pub(crate) fn ensure_private_parent(path: &Path) -> std::io::Result<()> {
    let path = path::normalized(path)?;
    let security = PrivateSecurity::new_directory()?;
    guard::with_ancestor_guards(&path, true, Some(&security), |guards| {
        security::validate_private_acl(guards.leaf()?)
    })
}

pub(crate) fn ensure_user_parent(path: &Path) -> std::io::Result<()> {
    let path = path::normalized(path)?;
    let security = PrivateSecurity::new_directory()?;
    guard::with_ancestor_guards(&path, true, Some(&security), |_| Ok(()))
}

pub(super) fn validate_private(file: &File) -> std::io::Result<()> {
    let info = file_information(file)?;
    if info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        return Err(super::permission_denied(
            "private path must be a regular non-reparse file",
        ));
    }
    if info.nNumberOfLinks != 1 {
        return Err(super::permission_denied(
            "private file must not be hard-linked",
        ));
    }
    validate_local(file)?;
    security::validate_private_acl(file)
}

pub(super) fn validate_regular(file: &File) -> std::io::Result<()> {
    let info = file_information(file)?;
    if info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        return Err(super::permission_denied(
            "path must be a regular non-reparse file",
        ));
    }
    validate_local(file)
}

pub(super) fn validate_local(file: &File) -> std::io::Result<()> {
    let mut remote = remote_protocol_query();
    let success = unsafe {
        GetFileInformationByHandleEx(
            raw_handle(file),
            FileRemoteProtocolInfo,
            (&raw mut remote).cast(),
            size_of::<FILE_REMOTE_PROTOCOL_INFO>() as u32,
        )
    };
    let error = if success == 0 {
        unsafe { GetLastError() }
    } else {
        0
    };
    classify_locality(success != 0, error)
}

fn classify_locality(success: bool, error: u32) -> std::io::Result<()> {
    if success {
        return Err(super::permission_denied(
            "network filesystems are not accepted here",
        ));
    }
    if error == ERROR_INVALID_PARAMETER {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "cannot verify that the Windows storage is local",
        ))
    }
}

pub(super) fn replace_atomic(
    source_file: &File,
    source: &Path,
    destination: &Path,
) -> std::io::Result<()> {
    let source = path::normalized(source)?;
    let destination = path::normalized(destination)?;
    path::validate_file_name(&source)?;
    path::validate_file_name(&destination)?;
    let source_parent = parent(&source)?;
    let destination_parent = parent(&destination)?;
    if source_parent == destination_parent {
        return guard::with_ancestor_guards(source_parent, false, None, |_guards| {
            rename::replace(source_file, &destination)
        });
    }
    guard::with_ancestor_guards(source_parent, false, None, |_source_guards| {
        guard::with_ancestor_guards(destination_parent, false, None, |_destination_guards| {
            rename::replace(source_file, &destination)
        })
    })
}

fn open_private(
    path: &Path,
    access: u32,
    creation: u32,
    create_parent: bool,
) -> std::io::Result<File> {
    let path = path::normalized(path)?;
    path::validate_file_name(&path)?;
    let (directory_security, file_security) = if create_parent {
        let (directory, file) = PrivateSecurity::new_pair()?;
        (Some(directory), file)
    } else {
        (None, PrivateSecurity::new_file()?)
    };
    with_leaf_parent(&path, create_parent, directory_security.as_ref(), || {
        let file = FileOpen::leaf(&path, access, creation, file_security.attributes()).execute()?;
        validate_private(&file)?;
        Ok(file)
    })
}

fn with_leaf_parent<T>(
    path: &Path,
    create_parent: bool,
    security: Option<&PrivateSecurity>,
    operation: impl FnOnce() -> std::io::Result<T>,
) -> std::io::Result<T> {
    guard::with_ancestor_guards(parent(path)?, create_parent, security, |_guards| {
        operation()
    })
}

fn create_private_directory(path: &Path, security: &PrivateSecurity) -> std::io::Result<()> {
    let path = path::wide_normalized(path)?;
    if unsafe { CreateDirectoryW(path.as_ptr(), security.attributes()) } != 0 {
        return Ok(());
    }
    let code = unsafe { GetLastError() };
    if code == ERROR_ALREADY_EXISTS {
        Ok(())
    } else {
        Err(std::io::Error::from_raw_os_error(code as i32))
    }
}

fn open_guarded_directory(path: &Path) -> std::io::Result<File> {
    FileOpen::guarded_directory(path, GENERIC_READ | READ_CONTROL, OPEN_EXISTING).execute()
}

fn validate_directory(file: &File) -> std::io::Result<()> {
    let info = file_information(file)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(super::permission_denied(
            "private file parent must be a real non-reparse directory",
        ));
    }
    validate_local(file)
}

fn file_information(file: &File) -> std::io::Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(raw_handle(file), &raw mut info) } == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(info)
    }
}

fn raw_handle(file: &File) -> HANDLE {
    file.as_raw_handle()
}

fn parent(path: &Path) -> std::io::Result<&Path> {
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| invalid_input("private file path has no parent"))
}

fn remote_protocol_query() -> FILE_REMOTE_PROTOCOL_INFO {
    FILE_REMOTE_PROTOCOL_INFO {
        StructureVersion: 2,
        StructureSize: size_of::<FILE_REMOTE_PROTOCOL_INFO>() as u16,
        ..Default::default()
    }
}

fn invalid_input(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ancestor_guards_exclude_delete_sharing() {
        assert_ne!(
            windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT,
            0
        );
        assert_eq!(LEAF_SHARING, FILE_SHARE_READ | FILE_SHARE_WRITE);
        assert_eq!(
            LEAF_SHARING & windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
            0
        );
        assert_eq!(GUARD_SHARING, FILE_SHARE_READ);
        assert_eq!(GUARD_SHARING & FILE_SHARE_WRITE, 0);
        assert_eq!(
            GUARD_SHARING & windows_sys::Win32::Storage::FileSystem::FILE_SHARE_DELETE,
            0
        );
        assert_ne!(TEMP_ACCESS & DELETE, 0);
    }

    #[test]
    fn remote_protocol_query_uses_the_current_contract_shape() {
        let query = remote_protocol_query();

        assert_eq!(query.StructureVersion, 2);
        assert_eq!(
            usize::from(query.StructureSize),
            size_of::<FILE_REMOTE_PROTOCOL_INFO>()
        );
        assert_eq!(query.Protocol, 0);
        assert_eq!(ERROR_INVALID_PARAMETER, 87);
        assert!(classify_locality(false, ERROR_INVALID_PARAMETER).is_ok());
        assert_eq!(
            classify_locality(true, 0).unwrap_err().kind(),
            std::io::ErrorKind::PermissionDenied
        );
        assert_eq!(
            classify_locality(false, 5).unwrap_err().kind(),
            std::io::ErrorKind::Unsupported
        );
    }
}
