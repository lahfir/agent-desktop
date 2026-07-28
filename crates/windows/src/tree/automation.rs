use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use crate::system::permissions::{com_hresult_detail, ensure_budget};

pub const ERR_NONE: i32 = 0;
pub const ERR_NOTFOUND: i32 = 1;
pub const ERR_TIMEOUT: i32 = 2;
pub const ERR_INACTIVE: i32 = 3;
pub const ERR_TYPE: i32 = 4;
pub const ERR_NULL_PTR: i32 = 5;
pub const ERR_FORMAT: i32 = 6;
pub const ERR_INVALID_OBJECT: i32 = 7;
pub const ERR_ALREADY_RUNNING: i32 = 8;
pub const ERR_INVALID_ARG: i32 = 9;

const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;
const E_POINTER: i32 = 0x8000_4003_u32 as i32;
const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;
const CO_E_NOTINITIALIZED: i32 = 0x8004_01F0_u32 as i32;
const RPC_E_SERVERFAULT: i32 = 0x8001_0105_u32 as i32;
const RPC_E_DISCONNECTED: i32 = 0x8001_0108_u32 as i32;
const RPC_S_SERVER_UNAVAILABLE: i32 = 0x8007_06BA_u32 as i32;
const RPC_S_CALL_FAILED: i32 = 0x8007_06BE_u32 as i32;
const UIA_E_ELEMENTNOTENABLED: i32 = 0x8004_0200_u32 as i32;
const UIA_E_ELEMENTNOTAVAILABLE: i32 = 0x8004_0201_u32 as i32;
const UIA_E_NOTSUPPORTED: i32 = 0x8004_0204_u32 as i32;
const UIA_E_TIMEOUT: i32 = 0x8013_1505_u32 as i32;
const UIA_E_INVALIDOPERATION: i32 = 0x8013_1509_u32 as i32;

const COM_UNINITIALIZED_SUGGESTION: &str =
    "Join the calling thread to the COM multithreaded apartment before observing the desktop";

/// Longest this crate will wait to learn whether a window thread is pumping.
///
/// Bounded independently of the operation deadline: the probe exists to avoid
/// an unbounded block, so spending the whole remaining budget on it would
/// defeat its purpose.
const PUMP_PROBE_CAP_MS: u64 = 2_000;

/// Reports a window whose thread is not dispatching messages.
///
/// Distinct from a window that does not exist: the handle is valid, the
/// provider is simply unreachable, and retrying later can succeed.
pub fn unresponsive_window_error() -> AdapterError {
    AdapterError::new(
        ErrorCode::AppUnresponsive,
        "The window's thread is not dispatching messages",
    )
    .with_suggestion("Wait for the application to become responsive, then retry")
    .with_platform_detail("WM_GETOBJECT would block: the window thread did not answer WM_NULL")
}

/// One UI Automation failure, already split on the discriminator the crate's
/// `Error` type mixes into a single `i32`.
///
/// `uiautomation::Error` carries its own non-negative sentinels (`ERR_NONE`,
/// `ERR_NOTFOUND`, …) in the same field as a real HRESULT, so `code()` alone
/// is ambiguous. `result()` is the only honest branch: `Some` for an HRESULT,
/// `None` for a crate sentinel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiaFailure {
    Hresult(i32),
    Sentinel(i32),
}

impl UiaFailure {
    /// Reports whether this failure is the benign end-of-list signal a child
    /// enumeration must not mistake for a fault.
    ///
    /// `windows-core` returns `Error::empty()` for a null interface out-param,
    /// whose `code()` is `HRESULT(0)`, which `uiautomation` stores verbatim.
    /// The runtime pair is measured by the CI capability probe rather than
    /// inferred from that chain.
    pub fn is_exhaustion(self) -> bool {
        matches!(self, UiaFailure::Sentinel(ERR_NONE))
    }
}

/// Builds a structured adapter error from a UI Automation failure.
///
/// `context` is a caller-supplied, shape-only phrase. Nothing observed from
/// the target — a property value, a `Name`, a `ClassName`, a window title, or
/// a `ProviderDescription` — may reach this function, because
/// `ref_action.rs` clones adapter error text into session trace segments.
pub fn uia_failure_error(failure: UiaFailure, context: &str) -> AdapterError {
    match failure {
        UiaFailure::Hresult(hresult) => {
            let (code, suggestion) = hresult_disposition(hresult);
            let error = AdapterError::new(code, format!("UI Automation could not {context}"))
                .with_platform_detail(com_hresult_detail(hresult));
            match suggestion {
                Some(hint) => error.with_suggestion(hint),
                None => error,
            }
        }
        UiaFailure::Sentinel(sentinel) => AdapterError::new(
            sentinel_disposition(sentinel),
            format!("UI Automation could not {context}"),
        )
        .with_platform_detail(format!("UI Automation client status {sentinel}")),
    }
}

fn hresult_disposition(hresult: i32) -> (ErrorCode, Option<&'static str>) {
    match hresult {
        E_ACCESSDENIED => (ErrorCode::PermDenied, None),
        CO_E_NOTINITIALIZED => (ErrorCode::Internal, Some(COM_UNINITIALIZED_SUGGESTION)),
        E_INVALIDARG | E_POINTER => (ErrorCode::InvalidArgs, None),
        UIA_E_ELEMENTNOTAVAILABLE => (ErrorCode::StaleRef, None),
        UIA_E_ELEMENTNOTENABLED => (ErrorCode::ActionFailed, None),
        UIA_E_NOTSUPPORTED => (ErrorCode::ActionNotSupported, None),
        UIA_E_TIMEOUT => (ErrorCode::Timeout, None),
        UIA_E_INVALIDOPERATION => (ErrorCode::ActionFailed, None),
        RPC_E_DISCONNECTED | RPC_E_SERVERFAULT | RPC_S_SERVER_UNAVAILABLE | RPC_S_CALL_FAILED => {
            (ErrorCode::AppUnresponsive, None)
        }
        _ => (ErrorCode::Internal, None),
    }
}

fn sentinel_disposition(sentinel: i32) -> ErrorCode {
    match sentinel {
        ERR_NOTFOUND | ERR_NULL_PTR => ErrorCode::ElementNotFound,
        ERR_TIMEOUT => ErrorCode::Timeout,
        ERR_INACTIVE => ErrorCode::AppUnresponsive,
        ERR_INVALID_OBJECT => ErrorCode::StaleRef,
        ERR_INVALID_ARG => ErrorCode::InvalidArgs,
        ERR_NONE | ERR_TYPE | ERR_FORMAT | ERR_ALREADY_RUNNING => ErrorCode::Internal,
        _ => ErrorCode::Internal,
    }
}

/// Rewrites a root-resolution failure so a window that no longer exists is
/// reported as a missing window rather than a stale element.
pub fn root_resolution_error(failure: UiaFailure) -> AdapterError {
    let error = uia_failure_error(failure, "resolve a window root");
    match failure {
        UiaFailure::Hresult(UIA_E_ELEMENTNOTAVAILABLE) | UiaFailure::Sentinel(ERR_NOTFOUND) => {
            AdapterError::new(
                ErrorCode::WindowNotFound,
                "UI Automation could not resolve a window root",
            )
            .with_platform_detail(error.platform_detail.unwrap_or_default())
            .with_suggestion("List windows again and retry with a current window handle")
        }
        _ => error,
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{UiaFailure, root_resolution_error, uia_failure_error};
    use crate::tree::element::UIAElement;
    use agent_desktop_core::{AdapterError, Deadline};
    use std::cell::OnceCell;
    use uiautomation::{Error as UiaError, UIAutomation, types::Handle};

    thread_local! {
        static CLIENT: OnceCell<UIAutomation> = const { OnceCell::new() };
    }

    /// Splits a crate error onto the `result()` discriminator.
    pub fn failure_of(error: &UiaError) -> UiaFailure {
        match error.result() {
            Some(hresult) => UiaFailure::Hresult(hresult.0),
            None => UiaFailure::Sentinel(error.code()),
        }
    }

    pub fn uia_error(error: &UiaError, context: &str) -> AdapterError {
        uia_failure_error(failure_of(error), context)
    }

    /// Hands out this thread's UI Automation client.
    ///
    /// Constructed with `new_direct()` only. `new()` would call
    /// `CoInitializeEx` itself: on a thread already in the MTA that returns
    /// `S_FALSE` and permanently leaks one initialization count, and on any
    /// STA host thread it fails outright with `RPC_E_CHANGED_MODE`. Sub-phase
    /// 2.1's bootstrap owns the apartment, so this accessor asserts the
    /// precondition instead of establishing it.
    pub fn automation_client() -> Result<UIAutomation, AdapterError> {
        CLIENT.with(|cell| {
            if let Some(client) = cell.get() {
                return Ok(client.clone());
            }
            let client = UIAutomation::new_direct()
                .map_err(|error| uia_error(&error, "create a UI Automation client"))?;
            let _ = cell.set(client.clone());
            Ok(client)
        })
    }

    /// Resolves a top-level window handle to its UI Automation root element.
    ///
    /// `ElementFromHandle` sends `WM_GETOBJECT` to the target's window thread,
    /// and a cross-thread `SendMessage` has no timeout of its own. A `Deadline`
    /// checked around the call therefore **cannot** interrupt it: against a
    /// target that has stopped pumping, the call blocks and the deadline is
    /// only observed once it returns.
    ///
    /// So the target is asked first whether it is pumping at all, with
    /// `SendMessageTimeoutW(WM_NULL, SMTO_ABORTIFHUNG)` - the documented way to
    /// put a bound on exactly this question. A target that is already hung
    /// becomes a structured `APP_UNRESPONSIVE` instead of an indefinite block.
    ///
    /// This is a mitigation, not a guarantee: a target that stops pumping in
    /// the window between the probe and the call still blocks. Bounding that
    /// needs the call issued on a thread this sub-phase can abandon, which is
    /// a hang guard 2.2 does not own.
    pub fn root_from_hwnd(hwnd: isize, deadline: Deadline) -> Result<UIAElement, AdapterError> {
        crate::system::permissions::ensure_budget(deadline)?;
        let client = automation_client()?;
        if !window_exists(hwnd) {
            return Err(root_resolution_error(UiaFailure::Sentinel(
                super::ERR_NOTFOUND,
            )));
        }
        let probe_ms = deadline.remaining_ms().min(super::PUMP_PROBE_CAP_MS);
        if !window_is_pumping(hwnd, probe_ms) {
            return Err(super::unresponsive_window_error());
        }
        let element = client
            .element_from_handle(Handle::from(hwnd))
            .map_err(|error| root_resolution_error(failure_of(&error)))?;
        crate::system::permissions::ensure_budget(deadline)?;
        Ok(UIAElement::from(element))
    }

    /// Reports whether a handle still addresses a live window.
    ///
    /// Asked before the pump probe so the two failures stay distinct: a handle
    /// that addresses nothing is a missing window, which is the mapping A14-5
    /// measured, while a live window that will not answer is an unresponsive
    /// application. Collapsing them would make a destroyed window look like a
    /// hung one and send a caller into a pointless retry.
    pub fn window_exists(hwnd: isize) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::IsWindow;
        hwnd != 0 && unsafe { IsWindow(hwnd as *mut std::ffi::c_void) } != 0
    }

    /// Asks whether a window's thread is dispatching messages, without
    /// blocking on it.
    ///
    /// `WM_NULL` is the no-op message every window proc handles, and
    /// `SMTO_ABORTIFHUNG` makes the call return immediately when the target is
    /// already known to be hung rather than waiting out the timeout.
    pub fn window_is_pumping(hwnd: isize, timeout_ms: u64) -> bool {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_NULL,
        };
        let mut answer: usize = 0;
        let sent = unsafe {
            SendMessageTimeoutW(
                hwnd as *mut std::ffi::c_void,
                WM_NULL,
                0,
                0,
                SMTO_ABORTIFHUNG,
                u32::try_from(timeout_ms.max(1)).unwrap_or(u32::MAX),
                &mut answer,
            )
        };
        sent != 0
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{UiaFailure, root_resolution_error};
    use crate::tree::element::{CannedElement, UIAElement};
    use agent_desktop_core::{AdapterError, Deadline};

    /// Canned arm so the classifier tests, and every module that calls the
    /// resolver, compile and run on a non-Windows lane.
    pub fn root_from_hwnd(hwnd: isize, deadline: Deadline) -> Result<UIAElement, AdapterError> {
        crate::system::permissions::ensure_budget(deadline)?;
        if hwnd == 0 {
            return Err(root_resolution_error(UiaFailure::Sentinel(
                super::ERR_NOTFOUND,
            )));
        }
        Ok(UIAElement::from(CannedElement))
    }
}

pub use imp::root_from_hwnd;

#[cfg(target_os = "windows")]
pub use imp::{automation_client, failure_of, uia_error, window_exists, window_is_pumping};

/// Rejects a window handle that cannot address a window before it reaches the
/// COM layer, so a null handle is an argument error rather than a COM failure.
pub fn validate_window_handle(hwnd: isize, deadline: Deadline) -> Result<(), AdapterError> {
    ensure_budget(deadline)?;
    if hwnd == 0 {
        return Err(
            AdapterError::new(ErrorCode::InvalidArgs, "Window handle is empty")
                .with_suggestion("Pass a window handle obtained from a window listing"),
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "automation_tests.rs"]
mod tests;
