use std::fs::File;
use std::mem::{offset_of, size_of};
use std::os::windows::io::AsRawHandle;
use std::path::Path;
use std::ptr::null_mut;
use windows_sys::Win32::Storage::FileSystem::{
    FILE_RENAME_INFO, FileRenameInfo, SetFileInformationByHandle,
};

pub(super) struct RenameBuffer {
    words: Vec<usize>,
    byte_len: u32,
}

impl RenameBuffer {
    fn new(destination: &Path) -> std::io::Result<Self> {
        let name = super::path::wide_normalized(destination)?;
        if name.last() != Some(&0) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Windows rename target is not NUL-terminated",
            ));
        }
        let name_units = name.len().checked_sub(1).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Windows rename target is not NUL-terminated",
            )
        })?;
        let name_bytes = name_units.checked_mul(size_of::<u16>()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Windows rename target is too large",
            )
        })?;
        let stored_name_bytes = name.len().checked_mul(size_of::<u16>()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Windows rename target is too large",
            )
        })?;
        let byte_len = offset_of!(FILE_RENAME_INFO, FileName)
            .checked_add(stored_name_bytes)
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Windows rename buffer is too large",
                )
            })?;
        let mut words = vec![0usize; byte_len.div_ceil(size_of::<usize>())];
        let info = words.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        unsafe {
            (*info).Anonymous.ReplaceIfExists = true;
            (*info).RootDirectory = null_mut();
            (*info).FileNameLength = name_bytes as u32;
            std::ptr::copy_nonoverlapping(
                name.as_ptr(),
                (&raw mut (*info).FileName).cast::<u16>(),
                name.len(),
            );
        }
        Ok(Self {
            words,
            byte_len: byte_len as u32,
        })
    }

    fn as_ptr(&self) -> *const std::ffi::c_void {
        self.words.as_ptr().cast()
    }
}

pub(super) fn replace(source: &File, destination: &Path) -> std::io::Result<()> {
    let rename = RenameBuffer::new(destination)?;
    if unsafe {
        SetFileInformationByHandle(
            source.as_raw_handle(),
            FileRenameInfo,
            rename.as_ptr(),
            rename.byte_len,
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error());
    }
    source.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replacement_contract_requires_the_open_source_descriptor() {
        let contract: fn(&File, &Path) -> std::io::Result<()> = replace;
        let _ = contract;
    }

    #[test]
    fn rename_layout_declares_the_terminal_nul_for_one_unit_basename() {
        let rename = RenameBuffer::new(Path::new(r"C:\x")).unwrap();
        let info = rename.as_ptr().cast::<FILE_RENAME_INFO>();
        let file_name_bytes = unsafe { (*info).FileNameLength } as usize;
        let nul_index = file_name_bytes / size_of::<u16>();
        let nul = unsafe { *(&raw const (*info).FileName).cast::<u16>().add(nul_index) };

        assert!(rename.byte_len as usize >= size_of::<FILE_RENAME_INFO>());
        assert!(
            offset_of!(FILE_RENAME_INFO, FileName) + file_name_bytes + size_of::<u16>()
                <= rename.byte_len as usize
        );
        assert_eq!(nul, 0);
    }
}
