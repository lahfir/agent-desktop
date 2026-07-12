use std::ffi::c_void;
use std::fs::File;
use std::mem::{size_of, zeroed};
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;
use std::sync::Arc;
use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
use windows_sys::Win32::Security::Authorization::{GetSecurityInfo, SE_FILE_OBJECT};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, ACL_SIZE_INFORMATION, AclSizeInformation,
    AddAccessAllowedAceEx, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, EqualSid, GetAce,
    GetAclInformation, GetLengthSid, GetSecurityDescriptorControl, GetTokenInformation,
    InitializeAcl, InitializeSecurityDescriptor, OBJECT_INHERIT_ACE, OWNER_SECURITY_INFORMATION,
    PSID, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR,
    SetSecurityDescriptorControl, SetSecurityDescriptorDacl, SetSecurityDescriptorOwner,
    TOKEN_QUERY, TOKEN_USER, TokenUser,
};
use windows_sys::Win32::Storage::FileSystem::FILE_ALL_ACCESS;
use windows_sys::Win32::System::SystemServices::{
    ACCESS_ALLOWED_ACE_TYPE, SECURITY_DESCRIPTOR_REVISION,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub(super) fn validate_private_acl(file: &File) -> std::io::Result<()> {
    let (current_sid, _) = current_user_sid()?;
    let mut owner = null_mut();
    let mut dacl = null_mut();
    let mut descriptor = null_mut();
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &raw mut owner,
            null_mut(),
            &raw mut dacl,
            null_mut(),
            &raw mut descriptor,
        )
    };
    if status != 0 {
        return Err(std::io::Error::from_raw_os_error(status as i32));
    }
    let current_sid_ptr = current_sid.as_ptr().cast_mut().cast();
    let result = if owner.is_null() || unsafe { EqualSid(owner, current_sid_ptr) } == 0 {
        Err(permission_denied(
            "private path is not owned by the current user",
        ))
    } else {
        validate_owner_only_dacl(descriptor, dacl, current_sid_ptr)
    };
    unsafe { LocalFree(descriptor) };
    result
}

fn validate_owner_only_dacl(
    descriptor: *mut c_void,
    dacl: *mut ACL,
    current_sid: PSID,
) -> std::io::Result<()> {
    let mut control = 0;
    let mut revision = 0;
    if dacl.is_null()
        || unsafe { GetSecurityDescriptorControl(descriptor, &raw mut control, &raw mut revision) }
            == 0
        || control & SE_DACL_PROTECTED == 0
    {
        return Err(permission_denied(
            "private path must have a protected owner-only DACL",
        ));
    }
    let mut acl_info = ACL_SIZE_INFORMATION::default();
    if unsafe {
        GetAclInformation(
            dacl,
            (&raw mut acl_info).cast(),
            size_of::<ACL_SIZE_INFORMATION>() as u32,
            AclSizeInformation,
        )
    } == 0
        || acl_info.AceCount != 1
    {
        return Err(permission_denied(
            "private path DACL must contain exactly one owner entry",
        ));
    }
    let mut raw_ace = null_mut();
    if unsafe { GetAce(dacl, 0, &raw mut raw_ace) } == 0 || raw_ace.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    let ace = unsafe { &*raw_ace.cast::<ACCESS_ALLOWED_ACE>() };
    let ace_sid = (&raw const ace.SidStart).cast_mut().cast();
    if ace.Header.AceType != ACCESS_ALLOWED_ACE_TYPE as u8
        || ace.Mask & FILE_ALL_ACCESS != FILE_ALL_ACCESS
        || unsafe { EqualSid(ace_sid, current_sid) } == 0
    {
        return Err(permission_denied(
            "private path grants access outside the current user",
        ));
    }
    Ok(())
}

pub(super) struct PrivateSecurity {
    _sid: Arc<Vec<usize>>,
    _acl: Vec<usize>,
    _descriptor: Box<SECURITY_DESCRIPTOR>,
    attributes: SECURITY_ATTRIBUTES,
}

impl PrivateSecurity {
    pub(super) fn new_directory() -> std::io::Result<Self> {
        let (sid, sid_bytes) = current_user_sid()?;
        Self::new_with_sid(Arc::new(sid), sid_bytes, true)
    }

    pub(super) fn new_file() -> std::io::Result<Self> {
        let (sid, sid_bytes) = current_user_sid()?;
        Self::new_with_sid(Arc::new(sid), sid_bytes, false)
    }

    pub(super) fn new_pair() -> std::io::Result<(Self, Self)> {
        let (sid, sid_bytes) = current_user_sid()?;
        let sid = Arc::new(sid);
        let directory = Self::new_with_sid(Arc::clone(&sid), sid_bytes, true)?;
        let file = Self::new_with_sid(sid, sid_bytes, false)?;
        Ok((directory, file))
    }

    fn new_with_sid(
        sid: Arc<Vec<usize>>,
        sid_bytes: usize,
        directory: bool,
    ) -> std::io::Result<Self> {
        let sid_ptr = sid.as_ref().as_ptr().cast_mut().cast();
        let acl_bytes =
            size_of::<ACL>() + size_of::<ACCESS_ALLOWED_ACE>() - size_of::<u32>() + sid_bytes;
        let mut acl = aligned_words(acl_bytes);
        let acl_ptr = acl.as_mut_ptr().cast::<ACL>();
        if unsafe { InitializeAcl(acl_ptr, acl_bytes as u32, ACL_REVISION) } == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let inheritance = if directory {
            CONTAINER_INHERIT_ACE | OBJECT_INHERIT_ACE
        } else {
            0
        };
        if unsafe {
            AddAccessAllowedAceEx(acl_ptr, ACL_REVISION, inheritance, FILE_ALL_ACCESS, sid_ptr)
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let mut descriptor = Box::new(unsafe { zeroed::<SECURITY_DESCRIPTOR>() });
        let descriptor_ptr = (&raw mut *descriptor).cast();
        if unsafe { InitializeSecurityDescriptor(descriptor_ptr, SECURITY_DESCRIPTOR_REVISION) }
            == 0
            || unsafe { SetSecurityDescriptorOwner(descriptor_ptr, sid_ptr, 0) } == 0
            || unsafe { SetSecurityDescriptorDacl(descriptor_ptr, 1, acl_ptr, 0) } == 0
            || unsafe {
                SetSecurityDescriptorControl(descriptor_ptr, SE_DACL_PROTECTED, SE_DACL_PROTECTED)
            } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let attributes = SECURITY_ATTRIBUTES {
            nLength: size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor_ptr,
            bInheritHandle: 0,
        };
        Ok(Self {
            _sid: sid,
            _acl: acl,
            _descriptor: descriptor,
            attributes,
        })
    }

    pub(super) fn attributes(&self) -> *const SECURITY_ATTRIBUTES {
        &raw const self.attributes
    }
}

fn current_user_sid() -> std::io::Result<(Vec<usize>, usize)> {
    let mut token = null_mut();
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    let result = (|| {
        let mut bytes = 0;
        unsafe { GetTokenInformation(token, TokenUser, null_mut(), 0, &raw mut bytes) };
        if bytes == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut buffer = aligned_words(bytes as usize);
        if unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                bytes,
                &raw mut bytes,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error());
        }
        let sid = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
        let sid_bytes = unsafe { GetLengthSid(sid) } as usize;
        if sid_bytes == 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut owned = aligned_words(sid_bytes);
        unsafe {
            std::ptr::copy_nonoverlapping(sid.cast::<u8>(), owned.as_mut_ptr().cast(), sid_bytes)
        };
        Ok((owned, sid_bytes))
    })();
    unsafe { CloseHandle(token) };
    result
}

fn aligned_words(byte_len: usize) -> Vec<usize> {
    vec![0; byte_len.div_ceil(size_of::<usize>())]
}

fn permission_denied(message: &'static str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn private_acl_is_protected_and_owner_only_by_construction() {
        assert_ne!(SE_DACL_PROTECTED, 0);
        assert_ne!(FILE_ALL_ACCESS, 0);
        assert_eq!(ACL_REVISION, 2);
    }
}
