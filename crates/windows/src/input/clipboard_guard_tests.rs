use super::MoveableMemory;
use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

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
