//! Ownership validation against the process token's `TokenOwner`.
//!
//! New filesystem objects land owned by `TokenOwner`, not `TokenUser`:
//! measured at both High and Medium integrity, a file created by an
//! admin-group account is owned by the Administrators group while
//! `OwnerMatchesTokenUser` is false and `OwnerMatchesTokenOwner` is true —
//! group membership is the variable, never integrity. Validation therefore
//! compares against `TokenOwner` only.
//!
//! The purpose is narrow: detecting a path pre-created by a foreign
//! principal, the Windows analogue of the unix uid post-condition in core's
//! `private_file.rs`. It is explicitly not an isolation boundary between
//! administrator processes — an administrator holds
//! `SeTakeOwnershipPrivilege`, so no file-permission mechanism can exclude
//! one. The unix `nlink` post-condition has no measured Windows analogue and
//! is deliberately not carried here.
//!
//! Only the owner is read — `GetSecurityInfo` with
//! `OWNER_SECURITY_INFORMATION` and no DACL requested — so this module reads
//! a fixed-layout SID and never touches an ACE.

use std::fs::File;
use std::io::ErrorKind;
use std::os::windows::io::AsRawHandle;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    EqualSid, GetLengthSid, GetTokenInformation, IsValidSid, OWNER_SECURITY_INFORMATION, PSID,
    SECURITY_MAX_SID_SIZE, TOKEN_INFORMATION_CLASS, TOKEN_OWNER, TOKEN_QUERY, TokenOwner,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

use super::permission_denied;

const TOKEN_OWNER_SIZE: usize = 8;
const _: () = assert!(size_of::<TOKEN_OWNER>() == TOKEN_OWNER_SIZE);

pub(super) struct SidBuffer {
    storage: Vec<u64>,
}

impl SidBuffer {
    pub(super) fn copied_from_valid(sid: PSID) -> std::io::Result<Self> {
        if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "the reported owner is not a valid SID",
            ));
        }
        let length = unsafe { GetLengthSid(sid) } as usize;
        if length == 0 || length > SECURITY_MAX_SID_SIZE as usize {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "the reported owner SID has an impossible length",
            ));
        }
        let mut storage = vec![0_u64; length.div_ceil(size_of::<u64>())];
        unsafe {
            std::ptr::copy_nonoverlapping(
                sid.cast::<u8>(),
                storage.as_mut_ptr().cast::<u8>(),
                length,
            );
        }
        Ok(Self { storage })
    }

    pub(super) fn as_psid(&self) -> PSID {
        self.storage.as_ptr().cast::<core::ffi::c_void>().cast_mut()
    }

    pub(super) fn matches(&self, other: &SidBuffer) -> bool {
        unsafe { EqualSid(self.as_psid(), other.as_psid()) != 0 }
    }
}

pub(super) fn require_owned_by_token_owner(file: &File, what: &str) -> std::io::Result<()> {
    let expected = process_token_owner_sid()?;
    let actual = actual_owner_for_comparison(file)?;
    if !expected.matches(&actual) {
        return Err(permission_denied(format!(
            "{what} is owned by a foreign principal, not this process's token owner"
        )));
    }
    Ok(())
}

fn actual_owner_for_comparison(file: &File) -> std::io::Result<SidBuffer> {
    #[cfg(test)]
    if forced_foreign_owner::is_active() {
        return forced_foreign_owner::foreign_sid();
    }
    file_owner_sid(file)
}

pub(super) fn file_owner_sid(file: &File) -> std::io::Result<SidBuffer> {
    let mut owner: PSID = std::ptr::null_mut();
    let mut descriptor: *mut core::ffi::c_void = std::ptr::null_mut();
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    let copied = SidBuffer::copied_from_valid(owner);
    if !descriptor.is_null() {
        unsafe { LocalFree(descriptor) };
    }
    copied
}

pub(super) fn process_token_owner_sid() -> std::io::Result<&'static SidBuffer> {
    static PROCESS_TOKEN_OWNER: OnceLock<Result<SidBuffer, String>> = OnceLock::new();
    PROCESS_TOKEN_OWNER
        .get_or_init(|| read_process_token_owner().map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|message| std::io::Error::new(ErrorKind::PermissionDenied, message.clone()))
}

fn read_process_token_owner() -> std::io::Result<SidBuffer> {
    let buffer = read_process_token_information(TokenOwner)?;
    let owner: TOKEN_OWNER = unsafe { std::ptr::read(buffer.as_ptr().cast()) };
    SidBuffer::copied_from_valid(owner.Owner)
}

fn read_process_token_information(class: TOKEN_INFORMATION_CLASS) -> std::io::Result<Vec<u64>> {
    let mut token: HANDLE = std::ptr::null_mut();
    let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
    if opened == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let information = read_token_information(token, class);
    unsafe { CloseHandle(token) };
    information
}

fn read_token_information(
    token: HANDLE,
    class: TOKEN_INFORMATION_CLASS,
) -> std::io::Result<Vec<u64>> {
    let mut required: u32 = 0;
    let probed =
        unsafe { GetTokenInformation(token, class, std::ptr::null_mut(), 0, &mut required) };
    if probed != 0 || required == 0 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "the process token reported no information payload",
        ));
    }
    let probe_error = std::io::Error::last_os_error();
    if probe_error.raw_os_error() != Some(ERROR_INSUFFICIENT_BUFFER as i32) {
        return Err(probe_error);
    }
    let mut buffer = vec![0_u64; (required as usize).div_ceil(size_of::<u64>())];
    let fetched = unsafe {
        GetTokenInformation(
            token,
            class,
            buffer.as_mut_ptr().cast(),
            required,
            &mut required,
        )
    };
    if fetched == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(buffer)
}

#[cfg(test)]
pub(super) fn process_token_user_sid_for_tests() -> std::io::Result<SidBuffer> {
    use windows_sys::Win32::Security::{TOKEN_USER, TokenUser};
    const TOKEN_USER_SIZE: usize = 16;
    const _: () = assert!(size_of::<TOKEN_USER>() == TOKEN_USER_SIZE);
    let buffer = read_process_token_information(TokenUser)?;
    let user: TOKEN_USER = unsafe { std::ptr::read(buffer.as_ptr().cast()) };
    SidBuffer::copied_from_valid(user.User.Sid)
}

/// Forces the owner comparison to observe a foreign principal so the
/// refusal branch of `require_owned_by_token_owner` can be exercised without
/// a file actually pre-created by another account. The substituted owner is
/// the `WinLocalSystemSid`, built programmatically so the seam stays portable
/// and privilege-free.
#[cfg(test)]
pub(super) mod forced_foreign_owner {
    use std::cell::Cell;

    use windows_sys::Win32::Security::{
        CreateWellKnownSid, SECURITY_MAX_SID_SIZE, WinLocalSystemSid,
    };

    use super::SidBuffer;

    thread_local! {
        static FORCE_FOREIGN_OWNER: Cell<bool> = const { Cell::new(false) };
    }

    pub(in super::super) fn is_active() -> bool {
        FORCE_FOREIGN_OWNER.with(Cell::get)
    }

    pub(in super::super) fn foreign_sid() -> std::io::Result<SidBuffer> {
        let mut storage = [0_u64; 9];
        let mut size: u32 = SECURITY_MAX_SID_SIZE;
        let created = unsafe {
            CreateWellKnownSid(
                WinLocalSystemSid,
                std::ptr::null_mut(),
                storage.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if created == 0 {
            return Err(std::io::Error::last_os_error());
        }
        SidBuffer::copied_from_valid(storage.as_mut_ptr().cast())
    }

    pub(in super::super) fn with_forced_foreign_owner<R>(run: impl FnOnce() -> R) -> R {
        struct ResetOnDrop;
        impl Drop for ResetOnDrop {
            fn drop(&mut self) {
                FORCE_FOREIGN_OWNER.with(|flag| flag.set(false));
            }
        }
        FORCE_FOREIGN_OWNER.with(|flag| flag.set(true));
        let _reset = ResetOnDrop;
        run()
    }
}
