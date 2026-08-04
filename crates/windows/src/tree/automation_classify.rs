//! HRESULT/sentinel failure classification.
//!
//! Turns one UI Automation failure - whether a COM HRESULT or a
//! `uiautomation` crate sentinel - into a structured `AdapterError` carrying
//! the typed `complete`/`retryable` disposition the resolution retry loop and
//! core's hydration read off the details payload.
//!
//! Split from `automation.rs` to separate this classification table from the
//! automation-client bootstrap and bounded-root resolution it feeds
//! (`automation.rs`): the table grows every time a newly-observed HRESULT or
//! sentinel needs a mapping, while the client plumbing does not change with
//! it.

use agent_desktop_core::{AdapterError, ErrorCode};

use crate::system::hresult::{
    CO_E_NOTINITIALIZED, E_ACCESSDENIED, E_INVALIDARG, E_POINTER, RPC_E_DISCONNECTED,
    RPC_E_SERVERFAULT, RPC_S_CALL_FAILED, RPC_S_SERVER_UNAVAILABLE, UIA_E_ELEMENTNOTAVAILABLE,
    UIA_E_ELEMENTNOTENABLED, UIA_E_INVALIDOPERATION, UIA_E_NOTSUPPORTED, UIA_E_TIMEOUT,
    com_hresult_detail,
};

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
///
/// Every error is stamped with the typed `complete`/`retryable` pair the
/// resolution retry loop and core's hydration read off the details payload
/// a settled absence or terminal failure is complete and never retried,
/// a transport failure or a vanished element is incomplete and retryable.
pub fn uia_failure_error(failure: UiaFailure, context: &str) -> AdapterError {
    let (complete, retryable) = uia_failure_disposition(failure).retry_details();
    let details = serde_json::json!({ "complete": complete, "retryable": retryable });
    match failure {
        UiaFailure::Hresult(hresult) => {
            let (code, suggestion) = hresult_disposition(hresult);
            let error = AdapterError::new(code, format!("UI Automation could not {context}"))
                .with_platform_detail(com_hresult_detail(hresult))
                .with_details(details);
            match suggestion {
                Some(hint) => error.with_suggestion(hint),
                None => error,
            }
        }
        UiaFailure::Sentinel(sentinel) => AdapterError::new(
            sentinel_disposition(sentinel),
            format!("UI Automation could not {context}"),
        )
        .with_platform_detail(format!("UI Automation client status {sentinel}"))
        .with_details(details),
    }
}

/// The read-path disposition of a UI Automation failure across both branches,
/// driving the `complete`/`retryable` stamp on `uia_failure_error`.
pub(crate) fn uia_failure_disposition(
    failure: UiaFailure,
) -> crate::system::hresult::ReadDisposition {
    match failure {
        UiaFailure::Hresult(hresult) => crate::system::hresult::classify_read_hresult(hresult),
        UiaFailure::Sentinel(sentinel) => match sentinel {
            ERR_TIMEOUT | ERR_INACTIVE => crate::system::hresult::ReadDisposition::Retryable,
            ERR_NOTFOUND | ERR_INVALID_OBJECT => {
                crate::system::hresult::ReadDisposition::Unavailable
            }
            ERR_INVALID_ARG => crate::system::hresult::ReadDisposition::SettledAbsence,
            _ => crate::system::hresult::ReadDisposition::Terminal,
        },
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

/// Split out so `automation.rs`'s tests can pin every named sentinel's
/// mapping without a catch-all guess; `pub(crate)` because that pin lives in
/// `automation`'s own test module, a sibling of this one rather than a
/// descendant.
pub(crate) fn sentinel_disposition(sentinel: i32) -> ErrorCode {
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
