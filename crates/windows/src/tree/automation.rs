use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

use crate::system::hresult::{
    CO_E_NOTINITIALIZED, E_ACCESSDENIED, E_INVALIDARG, E_POINTER, RPC_E_DISCONNECTED,
    RPC_E_SERVERFAULT, RPC_S_CALL_FAILED, RPC_S_SERVER_UNAVAILABLE, UIA_E_ELEMENTNOTAVAILABLE,
    UIA_E_ELEMENTNOTENABLED, UIA_E_INVALIDOPERATION, UIA_E_NOTSUPPORTED, UIA_E_TIMEOUT,
    com_hresult_detail,
};
use crate::system::permissions::ensure_budget;

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

const COM_UNINITIALIZED_SUGGESTION: &str =
    "Join the calling thread to the COM multithreaded apartment before observing the desktop";

/// Longest this crate will wait to learn whether a window thread is pumping.
///
/// Bounded independently of the operation deadline: the probe exists to avoid
/// an unbounded block, so spending the whole remaining budget on it would
/// defeat its purpose.
const PUMP_PROBE_CAP_MS: u64 = 2_000;

/// How long a UI Automation call may wait to reach a target's provider.
///
/// Measured, not assumed: against a window whose thread owns it but never
/// dispatches, `ElementFromHandle` returns `UIA_E_TIMEOUT` at this bound
/// instead of blocking. Without it the same call did not return inside a 30 s
/// watchdog.
pub const CONNECTION_TIMEOUT_MS: u32 = 2_000;

/// How long one UI Automation transaction may take end to end.
pub const TRANSACTION_TIMEOUT_MS: u32 = 20_000;

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
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};
    use windows::Win32::UI::Accessibility::{CUIAutomation8, IUIAutomation, IUIAutomation2};

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
    /// Constructed by direct `CoCreateInstance`, never through
    /// `UIAutomation::new()`. `new()` would call `CoInitializeEx` itself: on a
    /// thread already in the MTA that returns `S_FALSE` and permanently leaks
    /// one initialization count, and on any STA host thread it fails outright
    /// with `RPC_E_CHANGED_MODE`. Sub-phase 2.1's bootstrap owns the
    /// apartment, so this accessor asserts the precondition instead of
    /// establishing it. See `create_bounded_client` for which CLSID, and why.
    pub fn automation_client() -> Result<UIAutomation, AdapterError> {
        CLIENT.with(|cell| {
            if let Some(client) = cell.get() {
                return Ok(client.clone());
            }
            let client = create_bounded_client()?;
            let _ = cell.set(client.clone());
            Ok(client)
        })
    }

    /// Builds a client whose calls are bounded, and fails rather than hand
    /// back one whose calls are not.
    ///
    /// `UIAutomation::new_direct()` is `CoCreateInstance(&CUIAutomation, ...)`,
    /// and that object does not support `IUIAutomation2`, so its calls carry no
    /// timeout at all: measured against a window that stopped dispatching,
    /// `ElementFromHandle` did not return inside a 30 s watchdog.
    /// `CUIAutomation8` exposes `SetConnectionTimeout`, and the same call then
    /// returns `UIA_E_TIMEOUT` inside the bound.
    ///
    /// This keeps every property `new_direct()` was chosen for - it is the same
    /// direct `CoCreateInstance`, it never calls `CoInitializeEx`, so it works
    /// inside an STA host and leaks no initialization count in a long-lived
    /// process. Only the CLSID differs.
    ///
    /// There is deliberately **no fallback to the unbounded client**. A
    /// fallback would silently trade the hang guarantee for availability on a
    /// platform that cannot occur: `CUIAutomation8` has shipped since
    /// Windows 8, the product's floor is Windows 10 1809, and it is measured
    /// present on both build 17763 and the Server 2025 CI image. A client
    /// whose calls cannot be bounded is one this crate should refuse, not one
    /// it should quietly accept.
    fn create_bounded_client() -> Result<UIAutomation, AdapterError> {
        let client: IUIAutomation2 = unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_ALL) }
            .map_err(|error| unbounded_client_error(error.code().0))?;
        unsafe {
            client
                .SetConnectionTimeout(super::CONNECTION_TIMEOUT_MS)
                .map_err(|error| unbounded_client_error(error.code().0))?;
            client
                .SetTransactionTimeout(super::TRANSACTION_TIMEOUT_MS)
                .map_err(|error| unbounded_client_error(error.code().0))?;
        }
        let automation: IUIAutomation = client.into();
        Ok(UIAutomation::from(automation))
    }

    fn unbounded_client_error(hresult: i32) -> AdapterError {
        uia_failure_error(
            UiaFailure::Hresult(hresult),
            "create a UI Automation client whose calls are bounded",
        )
        .with_suggestion(
            "This build does not provide CUIAutomation8; observation is refused rather than run against a client that cannot time out",
        )
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
    /// The probe is the fast, precise answer, not the safety net. A target
    /// that answers it and then stops dispatching cannot be caught by any
    /// preflight, so the bound that actually holds is the client's own
    /// `ConnectionTimeout` (see `create_bounded_client`): that call returns
    /// `UIA_E_TIMEOUT` rather than blocking. The probe exists because it turns
    /// an already-hung target into a clearer error, sooner, than waiting the
    /// connection timeout out.
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
