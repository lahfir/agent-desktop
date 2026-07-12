use std::fs::File;
use std::os::windows::io::FromRawHandle;
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Foundation::{HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Security::SECURITY_ATTRIBUTES;
use windows_sys::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
};

pub(super) struct FileOpen<'a> {
    path: &'a Path,
    access: u32,
    creation: u32,
    directory: bool,
    sharing: u32,
    security: *const SECURITY_ATTRIBUTES,
}

impl<'a> FileOpen<'a> {
    pub(super) fn leaf(
        path: &'a Path,
        access: u32,
        creation: u32,
        security: *const SECURITY_ATTRIBUTES,
    ) -> Self {
        Self {
            path,
            access,
            creation,
            directory: false,
            sharing: super::LEAF_SHARING,
            security,
        }
    }

    pub(super) fn guarded_directory(path: &'a Path, access: u32, creation: u32) -> Self {
        Self {
            path,
            access,
            creation,
            directory: true,
            sharing: super::GUARD_SHARING,
            security: std::ptr::null(),
        }
    }

    pub(super) fn execute(self) -> std::io::Result<File> {
        let path = super::path::wide_normalized(self.path)?;
        let flags = FILE_FLAG_OPEN_REPARSE_POINT
            | if self.directory {
                FILE_FLAG_BACKUP_SEMANTICS
            } else {
                FILE_ATTRIBUTE_NORMAL
            };
        let handle = unsafe {
            CreateFileW(
                path.as_ptr(),
                self.access,
                self.sharing,
                self.security,
                self.creation,
                flags,
                null_mut(),
            )
        };
        file_from_handle(handle)
    }
}

fn file_from_handle(handle: HANDLE) -> std::io::Result<File> {
    if handle == INVALID_HANDLE_VALUE {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_handle(handle) })
    }
}
