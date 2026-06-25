use crate::convert::string::optional_adapter_string;
use crate::error::{self, AdResult};
use crate::ffi_try::{trap_panic, trap_panic_ptr, trap_panic_void};
use agent_desktop_core::context::{CommandContext, validate_session_id};
use agent_desktop_core::error::{AdapterError, AppError};
use agent_desktop_core::{PermissionState, adapter::PlatformAdapter};
use std::ffi::c_char;

pub struct AdAdapter {
    pub(crate) inner: Box<dyn PlatformAdapter>,
    pub(crate) session_id: Option<String>,
}

fn build_adapter() -> Box<dyn PlatformAdapter> {
    #[cfg(target_os = "macos")]
    {
        Box::new(agent_desktop_macos::MacOSAdapter::new())
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(agent_desktop_windows::WindowsAdapter::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(agent_desktop_linux::LinuxAdapter::new())
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    compile_error!("Unsupported platform")
}

/// Builds a platform adapter for the current OS and returns an opaque
/// handle. Returns null on allocation failure or if a Rust panic is
/// caught at the FFI boundary (inspect `ad_last_error_*` for details).
///
/// The returned pointer is owned by the caller and must be released with
/// `ad_adapter_destroy`. Creating and destroying adapters is cheap; the
/// common pattern is one adapter per process lifetime.
#[unsafe(no_mangle)]
pub extern "C" fn ad_adapter_create() -> *mut AdAdapter {
    trap_panic_ptr(|| {
        let adapter = AdAdapter {
            inner: build_adapter(),
            session_id: None,
        };
        Box::into_raw(Box::new(adapter))
    })
}

/// Builds a session-scoped platform adapter. `session` may be:
/// - null: equivalent to `ad_adapter_create()` (no session).
/// - a valid session id (1-64 ASCII alphanumeric / `-` / `_` chars): associates
///   the adapter with that session for refmap persistence.
/// - empty, too long, containing invalid characters, or invalid UTF-8: sets
///   `ErrInvalidArgs` in the last-error slot and returns null; no adapter is
///   allocated.
///
/// The returned pointer must be released with `ad_adapter_destroy`.
///
/// # Safety
///
/// `session` must be null or point to readable memory that is NUL-terminated
/// within `AD_MAX_STRING_BYTES + 1` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_adapter_create_with_session(session: *const c_char) -> *mut AdAdapter {
    trap_panic_ptr(|| {
        let session_id = match optional_adapter_string(session, "session") {
            Ok(opt) => opt,
            Err(err) => {
                error::set_last_error(&err);
                return std::ptr::null_mut();
            }
        };
        if let Some(id) = session_id.as_deref() {
            if let Err(app_err) = validate_session_id(id) {
                let adapter_err = match app_err {
                    AppError::Adapter(e) => e,
                    _ => AdapterError::new(
                        agent_desktop_core::error::ErrorCode::InvalidArgs,
                        "invalid session id",
                    ),
                };
                error::set_last_error(&adapter_err);
                return std::ptr::null_mut();
            }
        }
        let adapter = AdAdapter {
            inner: build_adapter(),
            session_id,
        };
        Box::into_raw(Box::new(adapter))
    })
}

/// # Safety
///
/// `adapter` must be a pointer returned by `ad_adapter_create`, or null.
/// After this call the pointer is invalid and must not be used.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_adapter_destroy(adapter: *mut AdAdapter) {
    trap_panic_void(|| {
        if !adapter.is_null() {
            drop(unsafe { Box::from_raw(adapter) });
        }
    })
}

/// # Safety
///
/// `adapter` must be a non-null pointer returned by `ad_adapter_create` that
/// has not yet been destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ad_check_permissions(adapter: *const AdAdapter) -> AdResult {
    trap_panic(|| {
        crate::pointer_guard::guard_non_null!(adapter, c"adapter is null");
        let adapter = unsafe { &*adapter };
        match adapter.inner.permission_report().accessibility {
            PermissionState::Granted => AdResult::Ok,
            PermissionState::Denied { suggestion } => {
                error::set_last_error(
                    &agent_desktop_core::error::AdapterError::new(
                        agent_desktop_core::error::ErrorCode::PermDenied,
                        "Accessibility permission not granted",
                    )
                    .with_suggestion(suggestion),
                );
                AdResult::ErrPermDenied
            }
            PermissionState::NotRequired => AdResult::Ok,
            PermissionState::Unknown => unknown_permission_result(adapter.inner.as_ref()),
        }
    })
}

impl AdAdapter {
    /// Builds a `CommandContext` from this adapter's session. Callers that
    /// need a context for context-taking commands (snapshot, status, wait)
    /// call this at the FFI entry boundary.
    pub fn command_context(&self) -> Result<CommandContext, AppError> {
        CommandContext::new(self.session_id.clone(), None, false)
    }
}

fn unknown_permission_result(adapter: &dyn PlatformAdapter) -> AdResult {
    let (code, message, suggestion) = if adapter.unknown_accessibility_means_unsupported() {
        (
            agent_desktop_core::error::ErrorCode::PlatformNotSupported,
            "Accessibility permission state is unknown because this platform adapter does not support permission probing",
            "Use a platform adapter with implemented permission probing before executing desktop actions.",
        )
    } else {
        (
            agent_desktop_core::error::ErrorCode::Internal,
            "Accessibility permission state is unknown",
            "Run the platform-specific permission report before executing desktop actions.",
        )
    };
    let err =
        agent_desktop_core::error::AdapterError::new(code, message).with_suggestion(suggestion);
    error::set_last_error(&err);
    crate::error::last_error_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_create_destroy() {
        let ptr = ad_adapter_create();
        assert!(!ptr.is_null());
        unsafe { ad_adapter_destroy(ptr) };
    }

    #[test]
    fn test_destroy_null_is_noop() {
        unsafe { ad_adapter_destroy(std::ptr::null_mut()) };
    }

    struct UnknownPermissionAdapter;

    impl PlatformAdapter for UnknownPermissionAdapter {
        fn permission_report(&self) -> agent_desktop_core::PermissionReport {
            agent_desktop_core::PermissionReport {
                accessibility: PermissionState::Unknown,
                screen_recording: PermissionState::Unknown,
                automation: PermissionState::NotRequired,
            }
        }
    }

    #[test]
    fn check_permissions_maps_default_unknown_accessibility_to_platform_unsupported() {
        let adapter = AdAdapter {
            inner: Box::new(UnknownPermissionAdapter),
            session_id: None,
        };

        let result = unsafe { ad_check_permissions(&adapter) };

        assert_eq!(result, AdResult::ErrPlatformNotSupported);
    }

    struct AmbiguousPermissionAdapter;

    impl PlatformAdapter for AmbiguousPermissionAdapter {
        fn permission_report(&self) -> agent_desktop_core::PermissionReport {
            agent_desktop_core::PermissionReport {
                accessibility: PermissionState::Unknown,
                screen_recording: PermissionState::Unknown,
                automation: PermissionState::NotRequired,
            }
        }

        fn unknown_accessibility_means_unsupported(&self) -> bool {
            false
        }
    }

    #[test]
    fn check_permissions_preserves_ambiguous_unknown_accessibility_as_internal() {
        let adapter = AdAdapter {
            inner: Box::new(AmbiguousPermissionAdapter),
            session_id: None,
        };

        let result = unsafe { ad_check_permissions(&adapter) };

        assert_eq!(result, AdResult::ErrInternal);
    }
}
