use agent_desktop_core::{AdapterError, Deadline};

use super::permissions::ensure_budget;
use windows_sys::Win32::System::Diagnostics::ToolHelp::THREADENTRY32;

/// One `TH32CS_SNAPTHREAD` walk of every thread on the desktop, feeding each
/// entry to `on_thread` until it returns `Some`, the walk runs out of
/// threads, or `deadline` expires. A caller that short-circuits on the first
/// match, like the single-pid menu-mode source, pays no budget check at all;
/// a caller that must see every thread, like the multi-pid source, pays
/// exactly one budget read per thread after the first.
///
/// The snapshot handle opened here is closed on every exit path this
/// function has: running out of threads, an early `Some` from `on_thread`,
/// and a deadline expiring mid-walk all close the same handle before
/// returning. A poller that repeats this walk every 200ms leaks one handle
/// per expired capture if any one of those paths skips the close.
pub(crate) fn walk_gui_threads<T>(
    deadline: Deadline,
    mut on_thread: impl FnMut(&THREADENTRY32) -> Option<T>,
) -> Result<Option<T>, AdapterError> {
    use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, Thread32First, Thread32Next,
    };

    #[cfg(test)]
    thread_snapshot_calls::record();

    let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) };
    // `CreateToolhelp32Snapshot` reports failure as INVALID_HANDLE_VALUE (-1),
    // never as a null handle, so an `is_null` guard can never fire and the
    // failure reads as an empty enumeration instead of an error.
    if snapshot == INVALID_HANDLE_VALUE {
        return Err(open_failure_error());
    }
    let mut entry = THREADENTRY32 {
        dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
        ..Default::default()
    };
    let mut found = None;
    let mut first = true;
    let mut ok = unsafe { Thread32First(snapshot, &mut entry) };
    while ok != 0 {
        if !first {
            if let Err(error) = ensure_budget(deadline) {
                unsafe { CloseHandle(snapshot) };
                #[cfg(test)]
                thread_snapshot_closes::record();
                return Err(error);
            }
        }
        first = false;
        if let Some(value) = on_thread(&entry) {
            found = Some(value);
            break;
        }
        ok = unsafe { Thread32Next(snapshot, &mut entry) };
    }
    unsafe { CloseHandle(snapshot) };
    #[cfg(test)]
    thread_snapshot_closes::record();
    Ok(found)
}

fn open_failure_error() -> AdapterError {
    super::listing_retry::narrow_to_permitted_codes(win32_last_error(
        "CreateToolhelp32Snapshot failed for classic menu-mode detection",
    ))
}

/// Win32 `GetLastError` -> `HRESULT_FROM_WIN32` into the shared HRESULT
/// table, the convention every native-failure module in this crate uses for
/// its own local errors.
fn win32_last_error(message: &str) -> AdapterError {
    let error = unsafe { windows_sys::Win32::Foundation::GetLastError() };
    adapter_error_from_win32(error, message)
}

fn adapter_error_from_win32(error: u32, message: &str) -> AdapterError {
    let hresult = super::process_state::hresult_from_win32(error);
    let record = super::hresult::hresult_record(hresult);
    let mut err = AdapterError::new(record.code, message)
        .with_platform_detail(super::hresult::com_hresult_detail(hresult));
    if let Some(suggestion) = record.suggestion {
        err = err.with_suggestion(suggestion);
    }
    err
}

/// Counts snapshot handle opens so a test can prove every `ToolHelp`
/// snapshot this walk creates is matched by exactly one close, across every
/// exit path.
#[cfg(test)]
pub(crate) mod thread_snapshot_calls {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(crate) fn take() -> usize {
        COUNT.with(|cell| {
            let value = cell.get();
            cell.set(0);
            value
        })
    }
}

/// Counts snapshot handle closes so a test can prove every opened `ToolHelp`
/// snapshot is closed on every exit path. The deadline check inside the walk
/// returns early, and an early return that skips the close leaks a handle per
/// expired capture on a loop that polls every 200ms.
#[cfg(test)]
pub(crate) mod thread_snapshot_closes {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<usize> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        COUNT.with(|cell| cell.set(cell.get() + 1));
    }

    pub(crate) fn take() -> usize {
        COUNT.with(|cell| {
            let value = cell.get();
            cell.set(0);
            value
        })
    }
}
