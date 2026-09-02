//! Who is on the other end of the control pipe.
//!
//! The pipe carries no security descriptor. This crate deliberately does not
//! author one — the module that would know how records descriptor authoring
//! and DACL validation as absent, after an earlier attempt sank on `AceSize`
//! handling, and a test pins the ACL/ACE symbol family out of it. So the peer
//! is authenticated instead, with the token APIs the crate already calls.
//!
//! The check runs in both directions. A server that only checked its clients
//! would still be a server a local process could impersonate: the pipe's name
//! is a deterministic function of the state root and session id, so anything
//! that wins the creation race receives every control the session sends and
//! can return the acknowledgement byte — which would make `data.rendered`
//! report `true` while nothing is drawn.
//!
//! What this does not close is stated rather than implied: every agent host
//! here is single-user, so a same-user process running the real binary passes
//! both checks. The security-descriptor decision is taken against that known
//! gap, not against a closed hole.

#[cfg(target_os = "windows")]
pub(crate) use imp::{peer_is_this_user, server_is_this_user};

#[cfg(not(target_os = "windows"))]
pub(crate) fn peer_is_this_user(_pipe: isize) -> bool {
    false
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn server_is_this_user(_pipe: isize) -> bool {
    false
}

#[cfg(target_os = "windows")]
mod imp {
    use crate::system::private_file::owner::SidBuffer;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::Security::{GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser};
    use windows_sys::Win32::System::Pipes::{
        GetNamedPipeClientProcessId, GetNamedPipeServerProcessId,
    };
    use windows_sys::Win32::System::Threading::{
        GetCurrentProcess, OpenProcess, OpenProcessToken, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    /// The client on this connection, resolved before its payload is read.
    ///
    /// `ImpersonateNamedPipeClient` is the obvious call and cannot serve
    /// here: it adopts the security context of the *last message read from
    /// the pipe*, so with nothing read there is no context to assume and it
    /// fails. Resolving the process id instead keeps the no-payload-read
    /// property the impersonation route only appeared to offer.
    pub(crate) fn peer_is_this_user(pipe: isize) -> bool {
        let mut process_id = 0u32;
        let ok = unsafe { GetNamedPipeClientProcessId(pipe as HANDLE, &mut process_id) };
        if ok == 0 {
            return false;
        }
        matches_this_user(process_id)
    }

    /// The server this client just connected to, checked before anything is
    /// written to it.
    pub(crate) fn server_is_this_user(pipe: isize) -> bool {
        let mut process_id = 0u32;
        let ok = unsafe { GetNamedPipeServerProcessId(pipe as HANDLE, &mut process_id) };
        if ok == 0 {
            return false;
        }
        matches_this_user(process_id)
    }

    fn matches_this_user(process_id: u32) -> bool {
        let Some(theirs) = user_sid_of(process_id) else {
            return false;
        };
        let Some(ours) = own_user_sid() else {
            return false;
        };
        ours.matches(&theirs)
    }

    fn own_user_sid() -> Option<SidBuffer> {
        let mut token: HANDLE = std::ptr::null_mut();
        let opened = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) };
        if opened == 0 {
            return None;
        }
        let sid = user_sid_of_token(token);
        unsafe { CloseHandle(token) };
        sid
    }

    fn user_sid_of(process_id: u32) -> Option<SidBuffer> {
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, process_id) };
        if process.is_null() {
            return None;
        }
        let mut token: HANDLE = std::ptr::null_mut();
        let opened = unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) };
        let sid = if opened == 0 {
            None
        } else {
            let sid = user_sid_of_token(token);
            unsafe { CloseHandle(token) };
            sid
        };
        unsafe { CloseHandle(process) };
        sid
    }

    fn user_sid_of_token(token: HANDLE) -> Option<SidBuffer> {
        let mut needed = 0u32;
        unsafe {
            GetTokenInformation(token, TokenUser, std::ptr::null_mut(), 0, &mut needed);
        }
        if needed == 0 {
            return None;
        }
        let mut buffer = vec![0u8; needed as usize];
        let ok = unsafe {
            GetTokenInformation(
                token,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            )
        };
        if ok == 0 {
            return None;
        }
        let user = unsafe { &*(buffer.as_ptr() as *const TOKEN_USER) };
        SidBuffer::copied_from_valid(user.User.Sid).ok()
    }
}
