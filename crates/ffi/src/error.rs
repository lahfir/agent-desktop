use agent_desktop_core::error::{AdapterError, ErrorCode};
use std::cell::RefCell;
use std::ffi::{c_char, CString};

#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdResult {
    Ok = 0,
    ErrPermDenied = -1,
    ErrElementNotFound = -2,
    ErrAppNotFound = -3,
    ErrActionFailed = -4,
    ErrActionNotSupported = -5,
    ErrStaleRef = -6,
    ErrWindowNotFound = -7,
    ErrPlatformNotSupported = -8,
    ErrTimeout = -9,
    ErrInvalidArgs = -10,
    ErrNotificationNotFound = -11,
    ErrInternal = -12,
}

struct StoredError {
    code: AdResult,
    message: CString,
    suggestion: Option<CString>,
    platform_detail: Option<CString>,
}

thread_local! {
    static LAST_ERROR: RefCell<Option<StoredError>> = const { RefCell::new(None) };
}

#[allow(dead_code)]
fn error_code_to_result(code: &ErrorCode) -> AdResult {
    match code {
        ErrorCode::PermDenied => AdResult::ErrPermDenied,
        ErrorCode::ElementNotFound => AdResult::ErrElementNotFound,
        ErrorCode::AppNotFound => AdResult::ErrAppNotFound,
        ErrorCode::ActionFailed => AdResult::ErrActionFailed,
        ErrorCode::ActionNotSupported => AdResult::ErrActionNotSupported,
        ErrorCode::StaleRef => AdResult::ErrStaleRef,
        ErrorCode::WindowNotFound => AdResult::ErrWindowNotFound,
        ErrorCode::PlatformNotSupported => AdResult::ErrPlatformNotSupported,
        ErrorCode::Timeout => AdResult::ErrTimeout,
        ErrorCode::InvalidArgs => AdResult::ErrInvalidArgs,
        ErrorCode::NotificationNotFound => AdResult::ErrNotificationNotFound,
        ErrorCode::Internal => AdResult::ErrInternal,
    }
}

#[allow(dead_code)]
pub(crate) fn set_last_error(err: &AdapterError) {
    let code = error_code_to_result(&err.code);
    let message = CString::new(err.message.as_str())
        .unwrap_or_else(|_| CString::new("(message contained null byte)").unwrap());
    let suggestion = err.suggestion.as_deref().and_then(|s| CString::new(s).ok());
    let platform_detail = err
        .platform_detail
        .as_deref()
        .and_then(|s| CString::new(s).ok());
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = Some(StoredError {
            code,
            message,
            suggestion,
            platform_detail,
        });
    });
}

#[allow(dead_code)]
pub(crate) fn clear_last_error() {
    LAST_ERROR.with(|cell| {
        *cell.borrow_mut() = None;
    });
}

#[allow(dead_code)]
pub(crate) fn last_error_code() -> AdResult {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|e| e.code)
            .unwrap_or(AdResult::Ok)
    })
}

#[allow(dead_code)]
pub(crate) fn last_error_message_str() -> Option<String> {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|e| e.message.to_string_lossy().into_owned())
    })
}

#[allow(dead_code)]
pub(crate) fn last_error_suggestion_str() -> Option<String> {
    LAST_ERROR.with(|cell| {
        cell.borrow().as_ref().and_then(|e| {
            e.suggestion
                .as_ref()
                .map(|s| s.to_string_lossy().into_owned())
        })
    })
}

#[allow(dead_code)]
pub(crate) fn last_error_platform_detail_str() -> Option<String> {
    LAST_ERROR.with(|cell| {
        cell.borrow().as_ref().and_then(|e| {
            e.platform_detail
                .as_ref()
                .map(|s| s.to_string_lossy().into_owned())
        })
    })
}

#[no_mangle]
pub extern "C" fn ad_last_error_code() -> AdResult {
    last_error_code()
}

#[no_mangle]
pub extern "C" fn ad_last_error_message() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .map(|e| e.message.as_ptr())
            .unwrap_or(std::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn ad_last_error_suggestion() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|e| e.suggestion.as_ref().map(|s| s.as_ptr()))
            .unwrap_or(std::ptr::null())
    })
}

#[no_mangle]
pub extern "C" fn ad_last_error_platform_detail() -> *const c_char {
    LAST_ERROR.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|e| e.platform_detail.as_ref().map(|s| s.as_ptr()))
            .unwrap_or(std::ptr::null())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_error_initially() {
        clear_last_error();
        assert!(last_error_message_str().is_none());
    }

    #[test]
    fn test_set_and_get_error() {
        let err = AdapterError::new(ErrorCode::ElementNotFound, "element @e5 gone")
            .with_suggestion("run snapshot");
        set_last_error(&err);
        assert_eq!(last_error_code(), AdResult::ErrElementNotFound);
        assert_eq!(last_error_message_str().unwrap(), "element @e5 gone");
        assert_eq!(last_error_suggestion_str().unwrap(), "run snapshot");
        assert!(last_error_platform_detail_str().is_none());
    }

    #[test]
    fn test_clear_error() {
        let err = AdapterError::internal("oops");
        set_last_error(&err);
        clear_last_error();
        assert!(last_error_message_str().is_none());
    }

    #[test]
    fn test_error_isolation_across_threads() {
        clear_last_error();
        let err = AdapterError::internal("thread1");
        set_last_error(&err);

        let handle = std::thread::spawn(|| last_error_message_str().is_none());
        assert!(handle.join().unwrap(), "other thread should see no error");
        assert_eq!(last_error_message_str().unwrap(), "thread1");
    }
}
