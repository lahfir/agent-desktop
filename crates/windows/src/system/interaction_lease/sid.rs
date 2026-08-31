//! SID reads and comparisons the lease's directory validation needs:
//! this process's token owner, token user, and mandatory integrity RID, plus
//! the well-known `SYSTEM` and `BUILTIN\Administrators` SIDs the private
//! lock directory's DACL is authored for.
//!
//! Deliberately separate from `system/private_file/owner.rs`: that module
//! accepts a token's full owner-eligible SID set (`TokenUser`, `TokenOwner`,
//! and any `SE_GROUP_OWNER`-flagged token group) for its narrower purpose
//! (detecting a path pre-created by a foreign principal), while the lease
//! directory here only ever needs to distinguish `TokenOwner` from
//! `TokenUser` directly - an elevated administrator's own token routinely
//! carries a `TokenOwner` of `BUILTIN\Administrators` distinct from its
//! `TokenUser`, and a directory this process creates is owned by whichever
//! one Windows defaults new objects to.

use std::io::ErrorKind;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_INSUFFICIENT_BUFFER, HANDLE, LocalFree};
use windows_sys::Win32::Security::Authorization::ConvertSidToStringSidW;
use windows_sys::Win32::Security::{
    CreateWellKnownSid, EqualSid, GetLengthSid, GetSidSubAuthority, GetSidSubAuthorityCount,
    GetTokenInformation, IsValidSid, PSID, SECURITY_MAX_SID_SIZE, TOKEN_INFORMATION_CLASS,
    TOKEN_MANDATORY_LABEL, TOKEN_OWNER, TOKEN_QUERY, TOKEN_USER, TokenIntegrityLevel, TokenOwner,
    TokenUser, WELL_KNOWN_SID_TYPE, WinBuiltinAdministratorsSid, WinLocalSystemSid,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub(super) struct SidBuffer {
    storage: Vec<u64>,
}

impl SidBuffer {
    pub(super) fn copied_from_valid(sid: PSID) -> std::io::Result<Self> {
        if sid.is_null() || unsafe { IsValidSid(sid) } == 0 {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "the reported SID is not valid",
            ));
        }
        let length = unsafe { GetLengthSid(sid) } as usize;
        if length == 0 || length > SECURITY_MAX_SID_SIZE as usize {
            return Err(std::io::Error::new(
                ErrorKind::InvalidData,
                "the reported SID has an impossible length",
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

    pub(super) fn well_known(sid_type: WELL_KNOWN_SID_TYPE) -> std::io::Result<Self> {
        let mut storage = [0_u64; (SECURITY_MAX_SID_SIZE as usize).div_ceil(size_of::<u64>())];
        let mut size = std::mem::size_of_val(&storage) as u32;
        let created = unsafe {
            CreateWellKnownSid(
                sid_type,
                std::ptr::null_mut(),
                storage.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if created == 0 {
            return Err(std::io::Error::last_os_error());
        }
        Self::copied_from_valid(storage.as_mut_ptr().cast())
    }

    pub(super) fn as_psid(&self) -> PSID {
        self.storage.as_ptr().cast::<core::ffi::c_void>().cast_mut()
    }

    pub(super) fn matches(&self, other: &SidBuffer) -> bool {
        unsafe { EqualSid(self.as_psid(), other.as_psid()) != 0 }
    }

    /// The textual `S-1-5-...` form, used as the per-user path segment under
    /// the lock root so two different token users never share a lock path.
    pub(super) fn to_string_form(&self) -> std::io::Result<String> {
        let mut wide: windows_sys::core::PWSTR = std::ptr::null_mut();
        let converted = unsafe { ConvertSidToStringSidW(self.as_psid(), &mut wide) };
        if converted == 0 || wide.is_null() {
            return Err(std::io::Error::last_os_error());
        }
        let text = unsafe { pwstr_to_string(wide) };
        unsafe { LocalFree(wide.cast()) };
        Ok(text)
    }
}

unsafe fn pwstr_to_string(wide: windows_sys::core::PWSTR) -> String {
    let mut length = 0usize;
    unsafe {
        while *wide.add(length) != 0 {
            length += 1;
        }
    }
    let slice = unsafe { std::slice::from_raw_parts(wide, length) };
    String::from_utf16_lossy(slice)
}

/// Process-invariant like the token SIDs below, so it is cached the same
/// way: every lease acquisition (`directory::ensure_private`'s
/// `validate_dacl` read-back runs on every acquisition, not only on first
/// creation) otherwise rebuilds this well-known SID from scratch on the hot
/// path every action command takes.
pub(super) fn well_known_system_sid() -> std::io::Result<&'static SidBuffer> {
    static CACHE: OnceLock<Result<SidBuffer, String>> = OnceLock::new();
    CACHE
        .get_or_init(|| SidBuffer::well_known(WinLocalSystemSid).map_err(|error| error.to_string()))
        .as_ref()
        .map_err(|message| std::io::Error::new(ErrorKind::PermissionDenied, message.clone()))
}

/// See [`well_known_system_sid`] - same process-invariant reasoning, same
/// cache shape.
pub(super) fn well_known_administrators_sid() -> std::io::Result<&'static SidBuffer> {
    static CACHE: OnceLock<Result<SidBuffer, String>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            SidBuffer::well_known(WinBuiltinAdministratorsSid).map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|message| std::io::Error::new(ErrorKind::PermissionDenied, message.clone()))
}

pub(super) fn process_token_owner_sid() -> std::io::Result<&'static SidBuffer> {
    static CACHE: OnceLock<Result<SidBuffer, String>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            read_process_token_information(TokenOwner)
                .and_then(|buffer| {
                    let owner: TOKEN_OWNER = unsafe { std::ptr::read(buffer.as_ptr().cast()) };
                    SidBuffer::copied_from_valid(owner.Owner)
                })
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|message| std::io::Error::new(ErrorKind::PermissionDenied, message.clone()))
}

pub(super) fn process_token_user_sid() -> std::io::Result<&'static SidBuffer> {
    static CACHE: OnceLock<Result<SidBuffer, String>> = OnceLock::new();
    CACHE
        .get_or_init(|| {
            read_process_token_information(TokenUser)
                .and_then(|buffer| {
                    let user: TOKEN_USER = unsafe { std::ptr::read(buffer.as_ptr().cast()) };
                    SidBuffer::copied_from_valid(user.User.Sid)
                })
                .map_err(|error| error.to_string())
        })
        .as_ref()
        .map_err(|message| std::io::Error::new(ErrorKind::PermissionDenied, message.clone()))
}

/// The caller's mandatory integrity RID (e.g. `0x2000` Medium, `0x1000` Low),
/// read from the last sub-authority of `TokenIntegrityLevel`'s label SID -
/// used only to annotate a `lease_integrity_denied` refusal, never to decide
/// policy.
pub(super) fn process_token_integrity_rid() -> std::io::Result<u32> {
    let buffer = read_process_token_information(TokenIntegrityLevel)?;
    let label: TOKEN_MANDATORY_LABEL = unsafe { std::ptr::read(buffer.as_ptr().cast()) };
    let sid = label.Label.Sid;
    let count = unsafe { *GetSidSubAuthorityCount(sid) };
    if count == 0 {
        return Err(std::io::Error::new(
            ErrorKind::InvalidData,
            "the integrity label SID has no sub-authorities",
        ));
    }
    let rid = unsafe { *GetSidSubAuthority(sid, u32::from(count) - 1) };
    Ok(rid)
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

/// A raw process token handle wrapper used by test-only privilege
/// manipulation (`AdjustTokenPrivileges`), which needs `TOKEN_ADJUST_PRIVILEGES`
/// rather than the `TOKEN_QUERY` this module otherwise opens with.
#[cfg(test)]
pub(super) fn open_process_token_for_adjust() -> std::io::Result<HANDLE> {
    use windows_sys::Win32::Security::TOKEN_ADJUST_PRIVILEGES;

    let mut token: HANDLE = std::ptr::null_mut();
    let opened =
        unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_ADJUST_PRIVILEGES, &mut token) };
    if opened == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(token)
}
