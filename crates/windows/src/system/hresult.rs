//! The one place a COM status code is named.
//!
//! Both the permission probe and the UI Automation tree path classify and
//! format HRESULTs. Holding two tables meant adding a code in one and reading
//! it as unnamed in the other, so the table lives here and both import it.

pub(crate) const S_OK: i32 = 0;
pub(crate) const E_NOINTERFACE: i32 = 0x8000_4002_u32 as i32;
pub(crate) const E_POINTER: i32 = 0x8000_4003_u32 as i32;
pub(crate) const E_FAIL: i32 = 0x8000_4005_u32 as i32;
pub(crate) const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;
pub(crate) const E_INVALIDARG: i32 = 0x8007_0057_u32 as i32;
pub(crate) const CO_E_NOTINITIALIZED: i32 = 0x8004_01F0_u32 as i32;
pub(crate) const RPC_E_SERVERFAULT: i32 = 0x8001_0105_u32 as i32;
pub(crate) const RPC_E_DISCONNECTED: i32 = 0x8001_0108_u32 as i32;
pub(crate) const RPC_S_SERVER_UNAVAILABLE: i32 = 0x8007_06BA_u32 as i32;
pub(crate) const RPC_S_CALL_FAILED: i32 = 0x8007_06BE_u32 as i32;
pub(crate) const UIA_E_ELEMENTNOTENABLED: i32 = 0x8004_0200_u32 as i32;
pub(crate) const UIA_E_ELEMENTNOTAVAILABLE: i32 = 0x8004_0201_u32 as i32;
pub(crate) const UIA_E_NOCLICKABLEPOINT: i32 = 0x8004_0202_u32 as i32;
pub(crate) const UIA_E_PROXYASSEMBLYNOTLOADED: i32 = 0x8004_0203_u32 as i32;
pub(crate) const UIA_E_NOTSUPPORTED: i32 = 0x8004_0204_u32 as i32;
pub(crate) const UIA_E_TIMEOUT: i32 = 0x8013_1505_u32 as i32;
pub(crate) const UIA_E_INVALIDOPERATION: i32 = 0x8013_1509_u32 as i32;

/// The read-path classification U4's retry loop consumes: whether a failed
/// read is a settled absence, a transient transport failure, a vanished
/// element, or a terminal error the attempt must surface as-is.
///
/// The three classes are the `kAXErrorIllegalArgument` lesson ported to
/// Windows (`crates/macos/src/system/window_bridge.rs:54-103`): a
/// structurally-impossible answer - the element genuinely lacks the attribute,
/// the window genuinely has no match - is a **settled absence, never
/// retryable**; conflating it with a transport failure burns the whole budget
/// retrying what cannot succeed. Retryable transport turns the attempt
/// incomplete within its deadline; a vanished element is the granularity case
/// (`UIA_E_ELEMENTNOTAVAILABLE`) that settles as stale only for a read of the
/// resolved target and marks a mid-descent node incomplete instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReadDisposition {
    /// Structurally impossible: the provider answered that the property or
    /// pattern does not exist, or the request could not be a valid read.
    /// Settled, never retried.
    SettledAbsence,
    /// Transport or timeout: retryable within the operation's deadline.
    Retryable,
    /// The element is unavailable: stale for the resolved target, incomplete
    /// for a node vanishing mid-descent.
    Unavailable,
    /// A hard failure with no defined recovery (denial, unknown code).
    Terminal,
}

impl ReadDisposition {
    /// The typed channel pair U4's loop and core's hydration read: a settled
    /// absence or terminal failure is complete and never retried; transport
    /// and a vanished element are incomplete and retryable within the
    /// deadline.
    pub(crate) fn retry_details(self) -> (bool, bool) {
        match self {
            ReadDisposition::SettledAbsence | ReadDisposition::Terminal => (true, false),
            ReadDisposition::Retryable | ReadDisposition::Unavailable => (false, true),
        }
    }
}

/// Classifies a named read-path HRESULT onto the three-state disposition.
///
/// An unlisted code is `Terminal` - the loop retries only what is marked
/// retryable, so an unknown failure surfaces rather than being guessed.
pub(crate) fn classify_read_hresult(hresult: i32) -> ReadDisposition {
    match hresult {
        UIA_E_NOTSUPPORTED | E_INVALIDARG | E_POINTER => ReadDisposition::SettledAbsence,
        UIA_E_TIMEOUT | RPC_E_DISCONNECTED | RPC_E_SERVERFAULT | RPC_S_SERVER_UNAVAILABLE
        | RPC_S_CALL_FAILED => ReadDisposition::Retryable,
        UIA_E_ELEMENTNOTAVAILABLE => ReadDisposition::Unavailable,
        _ => ReadDisposition::Terminal,
    }
}

/// Renders an HRESULT for `platform_detail`.
///
/// Shape only: a code and, where one is known, its symbol and meaning. No
/// entry derives from an observed application.
pub(crate) fn com_hresult_detail(hresult: i32) -> String {
    let code = hresult as u32;
    match com_hresult_symbol(hresult) {
        Some((symbol, meaning)) => format!("COM HRESULT 0x{code:08X} ({symbol}: {meaning})"),
        None => format!("COM HRESULT 0x{code:08X}"),
    }
}

/// Names the HRESULTs this crate's COM paths can raise.
///
/// An unlisted code formats as a bare hexadecimal value rather than being
/// guessed at.
pub(crate) fn com_hresult_symbol(hresult: i32) -> Option<(&'static str, &'static str)> {
    let symbol = match hresult {
        E_ACCESSDENIED => ("E_ACCESSDENIED", "Access is denied"),
        E_NOINTERFACE => ("E_NOINTERFACE", "No such interface supported"),
        E_POINTER => ("E_POINTER", "Invalid pointer"),
        E_FAIL => ("E_FAIL", "Unspecified failure"),
        E_INVALIDARG => ("E_INVALIDARG", "One or more arguments are invalid"),
        CO_E_NOTINITIALIZED => ("CO_E_NOTINITIALIZED", "COM has not been initialized"),
        RPC_E_SERVERFAULT => ("RPC_E_SERVERFAULT", "The server raised an exception"),
        RPC_E_DISCONNECTED => ("RPC_E_DISCONNECTED", "The object invoked has disconnected"),
        RPC_S_SERVER_UNAVAILABLE => ("RPC_S_SERVER_UNAVAILABLE", "The RPC server is unavailable"),
        RPC_S_CALL_FAILED => ("RPC_S_CALL_FAILED", "The remote procedure call failed"),
        UIA_E_ELEMENTNOTENABLED => ("UIA_E_ELEMENTNOTENABLED", "The element is not enabled"),
        UIA_E_ELEMENTNOTAVAILABLE => ("UIA_E_ELEMENTNOTAVAILABLE", "The element is not available"),
        UIA_E_NOCLICKABLEPOINT => (
            "UIA_E_NOCLICKABLEPOINT",
            "The element has no clickable point",
        ),
        UIA_E_PROXYASSEMBLYNOTLOADED => (
            "UIA_E_PROXYASSEMBLYNOTLOADED",
            "The proxy assembly could not be loaded",
        ),
        UIA_E_NOTSUPPORTED => (
            "UIA_E_NOTSUPPORTED",
            "The requested operation is unsupported",
        ),
        UIA_E_TIMEOUT => ("UIA_E_TIMEOUT", "The operation timed out"),
        UIA_E_INVALIDOPERATION => ("UIA_E_INVALIDOPERATION", "The operation is not valid"),
        _ => return None,
    };
    Some(symbol)
}


#[cfg(test)]
#[path = "hresult_tests.rs"]
mod tests;