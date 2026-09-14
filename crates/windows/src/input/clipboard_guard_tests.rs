use super::MoveableMemory;
use agent_desktop_core::{DeliveryDisposition, ErrorCode};
use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

use crate::input::clipboard_formats::CF_UNICODETEXT;

#[test]
fn failed_transfer_path_still_owns_handle_for_drop_free() {
    let guard = MoveableMemory::from_bytes(b"payload").expect("alloc");
    assert!(!guard.was_transferred());
    let handle = guard.handle_for_test();
    drop(guard);
    let size = unsafe { GlobalSize(handle) };
    assert_eq!(
        size, 0,
        "Drop must GlobalFree a handle that never transferred"
    );
}

#[test]
fn successful_transfer_path_releases_without_freeing() {
    let guard = MoveableMemory::from_bytes(b"keep-me").expect("alloc");
    let handle = guard.handle_for_test();
    guard.release_without_free_for_test();
    let size = unsafe { GlobalSize(handle) };
    assert_eq!(
        size, 7,
        "released-without-free must leave the allocation alive"
    );
    let locked = unsafe { GlobalLock(handle) };
    assert!(!locked.is_null());
    let bytes = unsafe { std::slice::from_raw_parts(locked.cast::<u8>(), 7) };
    assert_eq!(bytes, b"keep-me");
    unsafe {
        let _ = GlobalUnlock(handle);
        let _ = GlobalFree(handle);
    }
}

#[test]
fn an_empty_payload_is_refused_before_any_allocation() {
    match MoveableMemory::from_bytes(&[]) {
        Err(error) => assert_eq!(error.code, ErrorCode::InvalidArgs),
        Ok(_) => panic!("an empty payload must be refused"),
    }
}

/// `SetClipboardData` requires the calling thread to have opened the
/// clipboard first; calling it here without `OpenClipboard` fails
/// deterministically (`ERROR_CLIPBOARD_NOT_OPEN`) regardless of what any
/// other thread on the box is doing, so this needs no clipboard-ownership
/// lock the way a real content round-trip would.
#[test]
fn set_clipboard_data_without_an_open_clipboard_fails_and_drop_still_frees_the_handle() {
    let guard = MoveableMemory::from_bytes(b"unopened").expect("alloc");
    let handle = guard.handle_for_test();

    let error = guard
        .set_clipboard_data(CF_UNICODETEXT)
        .expect_err("SetClipboardData must fail without an open clipboard");

    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(
        error.disposition.delivery(),
        DeliveryDisposition::DeliveredUnverified
    );
    assert!(
        error
            .platform_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("GetLastError=")),
        "platform_detail must carry the Win32 error code: {:?}",
        error.platform_detail
    );

    let size = unsafe { GlobalSize(handle) };
    assert_eq!(
        size, 0,
        "a failed transfer must leave transferred=false so Drop frees the handle"
    );
}
