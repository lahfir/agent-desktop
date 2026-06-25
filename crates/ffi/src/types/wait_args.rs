use std::os::raw::c_char;

/// Arguments for `ad_wait`, mirroring `core::commands::wait::WaitArgs`.
///
/// Fields map as follows:
/// - `Option<u64>` → `u64` value + `bool has_*` sentinel (ms, count).
/// - `Option<String>` → nullable `*const c_char` (null = absent).
/// - `bool` → `bool`.
///
/// Callers must zero-initialize before use and verify layout via
/// `AD_WAIT_ARGS_SIZE` / `ad_wait_args_size()`.
#[repr(C)]
pub struct AdWaitArgs {
    /// Milliseconds to sleep (WaitMode::ms).
    pub ms: u64,
    pub has_ms: bool,

    /// Element ref id to wait for (WaitMode::element).
    pub element: *const c_char,

    /// Window title to wait for (WaitMode::window).
    pub window: *const c_char,

    /// Text to wait for (WaitMode::text / WaitMode::notification text).
    pub text: *const c_char,

    /// Wait for menu to open (true) or close (false via menu_closed).
    pub menu: bool,
    /// Wait for menu to close.
    pub menu_closed: bool,
    /// Wait for a notification.
    pub notification: bool,

    /// Snapshot id for element predicate (WaitPredicateArgs::snapshot_id).
    pub snapshot_id: *const c_char,

    /// Predicate kind string (WaitPredicateArgs::predicate).
    pub predicate: *const c_char,

    /// Expected value for value-predicate (WaitPredicateArgs::value).
    pub value: *const c_char,

    /// Action name for actionability-predicate (WaitPredicateArgs::action).
    pub action: *const c_char,

    /// Expected match count for text waits (WaitPredicateArgs::count).
    pub count: usize,
    pub has_count: bool,

    /// Timeout in milliseconds.
    pub timeout_ms: u64,

    /// App name filter (null = any). Maps to WaitArgs::app.
    pub app: *const c_char,
}

/// Pinned size of `AdWaitArgs` on 64-bit targets. The compile-time
/// assert below and the `ad_wait_args_size()` runtime getter form the
/// 3-layer pin: Rust const assert, C `_Static_assert` in the header,
/// and the test in `c_abi_layout.rs`.
pub const AD_WAIT_ARGS_SIZE: usize = 112;

const _: () = assert!(std::mem::size_of::<AdWaitArgs>() == AD_WAIT_ARGS_SIZE);

/// Returns the size of `AdWaitArgs` as compiled. Ctypes and other
/// foreign bindings must call this and compare against their own
/// `sizeof` before passing args to `ad_wait`.
#[unsafe(no_mangle)]
pub extern "C" fn ad_wait_args_size() -> usize {
    std::mem::size_of::<AdWaitArgs>()
}
