//! C-ABI contract harness.
//!
//! Drives the FFI exactly as a C program would — raw `extern "C"`
//! declarations with `#[allow(improper_ctypes)]` over the opaque
//! handle types, no high-level Rust access. Exercises bug classes the
//! inline `#[cfg(test)]` modules can't reach on their own:
//!
//! 1. Struct layouts that a consumer memcpy-copies (AdRect, AdAction).
//! 2. Enum fuzzing — `int32_t` bit patterns written into enum-typed
//!    fields must not UB, must return `AD_RESULT_ERR_INVALID_ARGS`.
//! 3. Null tolerance in the free-* family and accessor family.
//! 4. Interior-NUL inputs funnel through `string_to_c_lossy` without
//!    returning null.
//! 5. List handle lifecycle (count on null, _free on null, _get OOB).

#![allow(improper_ctypes)]

use agent_desktop_ffi::error::AdResult;
use agent_desktop_ffi::{
    AdAction, AdActionResult, AdAdapter, AdAppList, AdDirection, AdDragParams, AdFindQuery,
    AdKeyCombo, AdNativeHandle, AdPoint, AdRect, AdRefEntry, AdScrollParams, AdWindowInfo,
    AdWindowList,
};
use std::ffi::CStr;
use std::os::raw::c_char;

extern "C" {
    fn ad_adapter_create() -> *mut AdAdapter;
    fn ad_adapter_destroy(adapter: *mut AdAdapter);
    fn ad_check_permissions(adapter: *const AdAdapter) -> AdResult;

    fn ad_last_error_code() -> AdResult;
    fn ad_last_error_message() -> *const c_char;

    fn ad_list_apps(adapter: *const AdAdapter, out: *mut *mut AdAppList) -> AdResult;
    fn ad_app_list_count(list: *const AdAppList) -> u32;
    fn ad_app_list_get(list: *const AdAppList, index: u32) -> *const u8;
    fn ad_app_list_free(list: *mut AdAppList);

    fn ad_list_windows(
        adapter: *const AdAdapter,
        app_filter: *const c_char,
        focused_only: bool,
        out: *mut *mut AdWindowList,
    ) -> AdResult;
    fn ad_window_list_count(list: *const AdWindowList) -> u32;
    fn ad_window_list_free(list: *mut AdWindowList);

    fn ad_launch_app(
        adapter: *const AdAdapter,
        id: *const c_char,
        timeout_ms: u64,
        out: *mut AdWindowInfo,
    ) -> AdResult;

    fn ad_execute_action(
        adapter: *const AdAdapter,
        handle: *const AdNativeHandle,
        action: *const AdAction,
        out: *mut AdActionResult,
    ) -> AdResult;

    fn ad_find(
        adapter: *const AdAdapter,
        win: *const AdWindowInfo,
        query: *const AdFindQuery,
        out: *mut AdNativeHandle,
    ) -> AdResult;

    fn ad_free_handle(adapter: *const AdAdapter, handle: *mut AdNativeHandle) -> AdResult;

    fn ad_resolve_element(
        adapter: *const AdAdapter,
        entry: *const AdRefEntry,
        out: *mut AdNativeHandle,
    ) -> AdResult;
}

fn with_adapter<F: FnOnce(*mut AdAdapter)>(body: F) {
    unsafe {
        let adapter = ad_adapter_create();
        assert!(!adapter.is_null(), "ad_adapter_create must not return null");
        body(adapter);
        ad_adapter_destroy(adapter);
    }
}

fn default_scroll() -> AdScrollParams {
    AdScrollParams {
        direction: AdDirection::Down as i32,
        amount: 0,
    }
}

fn default_key() -> AdKeyCombo {
    AdKeyCombo {
        key: std::ptr::null(),
        modifiers: std::ptr::null(),
        modifier_count: 0,
    }
}

fn default_drag() -> AdDragParams {
    AdDragParams {
        from: AdPoint { x: 0.0, y: 0.0 },
        to: AdPoint { x: 0.0, y: 0.0 },
        duration_ms: 0,
    }
}

#[test]
fn rect_and_point_layouts_are_memcpyable() {
    // A C consumer that does { AdRect r; memcpy(&r, src, sizeof(r)); } must
    // see the same field values Rust wrote. Plain #[repr(C)] without
    // padding games makes this a byte copy.
    let rect = AdRect {
        x: 1.25,
        y: -2.5,
        width: 640.0,
        height: 480.0,
    };
    let copied = unsafe { std::ptr::read(&rect as *const AdRect) };
    assert_eq!(copied.x, 1.25);
    assert_eq!(copied.y, -2.5);
    assert_eq!(copied.width, 640.0);
    assert_eq!(copied.height, 480.0);

    let point = AdPoint { x: 3.0, y: 4.0 };
    let copied = unsafe { std::ptr::read(&point as *const AdPoint) };
    assert_eq!(copied.x, 3.0);
    assert_eq!(copied.y, 4.0);
}

#[test]
fn enum_fuzz_invalid_discriminant_rejected() {
    // AdAction.kind is `i32` — a buggy C caller can legally stuff any
    // value in here. Must not UB the Rust side; the validator should
    // surface AD_RESULT_ERR_INVALID_ARGS before any adapter code runs.
    with_adapter(|adapter| unsafe {
        let mut action: AdAction = std::mem::zeroed();
        action.kind = i32::MAX;
        action.scroll = default_scroll();
        action.key = default_key();
        action.drag = default_drag();
        action.text = std::ptr::null();

        let handle = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let mut out: AdActionResult = std::mem::zeroed();
        let rc = ad_execute_action(adapter, &handle, &action, &mut out);
        // Either the enum validator rejects (expected) or the cargo-test
        // worker thread trips the macOS main-thread assert and returns
        // ErrInternal. Both prove the absence of UB; what matters is that
        // the arbitrary bit pattern never results in AD_RESULT_OK.
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "arbitrary enum bit pattern must be rejected, got {:?}",
            rc
        );
    });
}

#[test]
fn null_adapter_rejected_without_ub() {
    unsafe {
        let mut list: *mut AdAppList = std::ptr::null_mut();
        let rc = ad_list_apps(std::ptr::null(), &mut list);
        // On cargo-test worker threads the macOS main-thread guard
        // fires first (ErrInternal); on the main thread the null-adapter
        // guard wins (ErrInvalidArgs). Either way: no dereference, no UB.
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));
        assert!(list.is_null(), "out-param must stay null on failure");

        let rc2 = ad_check_permissions(std::ptr::null());
        // ad_check_permissions has no main-thread guard — null adapter
        // must hit the null-check and return InvalidArgs deterministically.
        assert_eq!(rc2, AdResult::ErrInvalidArgs);
    }
}

#[test]
fn dirty_out_param_is_cleared_before_early_return_on_worker_thread() {
    // Regression for todo 006: prior shape ran require_main_thread()
    // *before* zeroing *out, so a worker-thread early-return left
    // whatever the caller had in the struct. A follow-up free on that
    // stale pointer would double-free. This test seeds the out slot
    // with fake pointers that *must* be cleared before the fn returns.
    with_adapter(|adapter| unsafe {
        let fake_ptr = 0xDEAD_BEEF as *mut AdAppList;
        let mut list: *mut AdAppList = fake_ptr;
        let rc = ad_list_apps(adapter, &mut list);
        // Either the main-thread guard (ErrInternal on worker) or a
        // successful call zeroed the slot before anything else.
        if rc != AdResult::Ok {
            assert!(
                list.is_null(),
                "dirty out-param must be zeroed before early return, got {:?}",
                list
            );
        }
    });
}

#[test]
fn invalid_utf8_filter_rejected_not_silently_widened() {
    // Regression for todo 010: prior c_to_string conflated null with
    // invalid UTF-8, so a non-null buffer with bogus bytes in the
    // app_filter slot would be treated as "no filter" and widen
    // ad_list_windows to every app on the system. Must now fail closed.
    with_adapter(|adapter| unsafe {
        let bad: [u8; 2] = [0xC3, 0x00];
        let mut list: *mut AdWindowList = std::ptr::null_mut();
        let rc = ad_list_windows(adapter, bad.as_ptr() as *const c_char, false, &mut list);
        // Main-thread guard (ErrInternal on worker) or UTF-8 rejection
        // (ErrInvalidArgs) — either way we do NOT produce a list by
        // silently treating bad bytes as "no filter".
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));
        assert!(list.is_null());
    });
}

#[test]
fn null_out_param_rejected_before_write() {
    with_adapter(|adapter| unsafe {
        let rc = ad_list_apps(adapter, std::ptr::null_mut());
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));
    });
}

#[test]
fn null_tolerance_on_list_accessors_and_free() {
    unsafe {
        assert_eq!(ad_app_list_count(std::ptr::null()), 0);
        assert!(ad_app_list_get(std::ptr::null(), 0).is_null());
        ad_app_list_free(std::ptr::null_mut());

        assert_eq!(ad_window_list_count(std::ptr::null()), 0);
        ad_window_list_free(std::ptr::null_mut());
    }
}

#[test]
fn list_handle_lifecycle_roundtrip() {
    with_adapter(|adapter| unsafe {
        let mut list: *mut AdAppList = std::ptr::null_mut();
        let rc = ad_list_apps(adapter, &mut list);
        // On CI without accessibility permission this may return PermDenied;
        // both outcomes preserve the contract we're testing (no UB, valid
        // pointer-or-null).
        if rc == AdResult::Ok {
            assert!(!list.is_null());
            let count = ad_app_list_count(list);
            // Request an index past the end — must return null, not segfault.
            assert!(ad_app_list_get(list, count).is_null());
            ad_app_list_free(list);
        } else {
            assert!(list.is_null(), "failed list call must leave out null");
            let msg_ptr = ad_last_error_message();
            assert!(!msg_ptr.is_null());
            let _ = CStr::from_ptr(msg_ptr).to_string_lossy();
            assert_eq!(ad_last_error_code(), rc);
        }
    });
}

#[test]
fn list_windows_focused_only_runs() {
    with_adapter(|adapter| unsafe {
        let mut list: *mut AdWindowList = std::ptr::null_mut();
        let rc = ad_list_windows(adapter, std::ptr::null(), true, &mut list);
        if rc == AdResult::Ok {
            assert!(!list.is_null());
            let _ = ad_window_list_count(list);
            ad_window_list_free(list);
        } else {
            assert!(list.is_null());
        }
    });
}

#[test]
fn invalid_utf8_app_id_rejected() {
    with_adapter(|adapter| unsafe {
        // A mid-byte of a multi-byte UTF-8 sequence, not valid on its own.
        let bad: [u8; 2] = [0xC3, 0];
        let mut out: AdWindowInfo = std::mem::zeroed();
        let rc = ad_launch_app(adapter, bad.as_ptr() as *const c_char, 0, &mut out);
        // Either the UTF-8 check (ErrInvalidArgs) or the macOS main-thread
        // guard (ErrInternal — cargo tests run on worker threads) rejects
        // the call. Either way no UB occurred.
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "must reject without UB, got {:?}",
            rc
        );
    });
}

#[test]
fn find_returns_not_found_on_empty_query_against_no_window() {
    with_adapter(|adapter| unsafe {
        let bad_win: AdWindowInfo = std::mem::zeroed();
        let query = AdFindQuery {
            role: std::ptr::null(),
            name_substring: std::ptr::null(),
            value_substring: std::ptr::null(),
        };
        let mut handle = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_find(adapter, &bad_win, &query, &mut handle);
        // A zeroed window has null id/title → InvalidArgs. On cargo-test
        // worker threads the main-thread assert trips first and returns
        // ErrInternal. Either outcome means the FFI refused a malformed
        // input without UB.
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "zeroed window must not succeed, got {:?}",
            rc
        );
    });
}

#[test]
fn free_handle_null_is_noop() {
    with_adapter(|adapter| unsafe {
        let mut handle = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_free_handle(adapter, &mut handle);
        assert_eq!(rc, AdResult::Ok);
        assert!(handle.ptr.is_null());

        let rc2 = ad_free_handle(adapter, std::ptr::null_mut());
        assert_eq!(rc2, AdResult::Ok);
    });
}

#[cfg(not(target_os = "macos"))]
#[test]
fn free_handle_zeroes_ptr_so_double_free_is_noop() {
    // macOS is excluded: ad_free_handle invokes CFRelease on the
    // underlying pointer, and a fabricated "fake live" pointer will
    // SIGBUS before we can observe the zeroing. On Windows/Linux
    // release_handle is NotSupported (no platform call), so the zeroing
    // contract is safely observable with a fake pointer.
    with_adapter(|adapter| unsafe {
        let fake_live_ptr = 0x1234 as *const std::ffi::c_void;
        let mut handle = AdNativeHandle { ptr: fake_live_ptr };

        let _ = ad_free_handle(adapter, &mut handle);
        assert!(handle.ptr.is_null());

        let rc = ad_free_handle(adapter, &mut handle);
        assert_eq!(rc, AdResult::Ok);
    });
}

#[test]
fn resolve_element_rejects_invalid_utf8_name() {
    // Regression for todo 004: prior c_to_string(entry.name) conflated
    // null with invalid UTF-8, so a non-null buffer with bogus bytes in
    // the `name` slot was treated as "no name filter" and widened the
    // re-resolution match. Must now fail closed with InvalidArgs.
    with_adapter(|adapter| unsafe {
        let role = std::ffi::CString::new("button").unwrap();
        let bad_name: [u8; 2] = [0xC3, 0x00]; // partial multi-byte + NUL
        let entry = AdRefEntry {
            pid: 0,
            role: role.as_ptr(),
            name: bad_name.as_ptr() as *const c_char,
            bounds_hash: 0,
            has_bounds_hash: false,
        };
        let mut out = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_resolve_element(adapter, &entry, &mut out);
        // Either the UTF-8 check wins (ErrInvalidArgs) or the main-thread
        // guard wins on worker threads (ErrInternal). Either way, no
        // silent widening, no UB, and the out-handle stays null.
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "must reject without UB, got {:?}",
            rc
        );
        assert!(out.ptr.is_null());
    });
}

#[test]
fn execute_action_rejects_null_handle_ptr() {
    with_adapter(|adapter| unsafe {
        let action = AdAction {
            kind: 0,
            text: std::ptr::null(),
            scroll: AdScrollParams {
                direction: 0,
                amount: 0,
            },
            key: AdKeyCombo {
                key: std::ptr::null(),
                modifiers: std::ptr::null(),
                modifier_count: 0,
            },
            drag: AdDragParams {
                from: AdPoint { x: 0.0, y: 0.0 },
                to: AdPoint { x: 0.0, y: 0.0 },
                duration_ms: 0,
            },
        };
        let handle = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let mut out: AdActionResult = std::mem::zeroed();
        let rc = ad_execute_action(adapter, &handle, &action, &mut out);
        // Main-thread guard or null-ptr guard wins; both avoid UB and
        // keep the struct-level handle accepted while rejecting the
        // inner null pointer.
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));
    });
}

#[test]
fn last_error_survives_successful_calls() {
    with_adapter(|adapter| unsafe {
        let mut out: AdWindowInfo = std::mem::zeroed();
        let rc = ad_launch_app(adapter, std::ptr::null(), 0, &mut out);
        // Worker-thread cargo tests hit the main-thread guard first,
        // producing ErrInternal. Main-thread runs would see
        // ErrInvalidArgs. What we need is that *some* failure populates
        // last-error and its pointer stays stable across the success
        // calls that follow.
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "must fail, got {:?}",
            rc
        );
        let msg_ptr = ad_last_error_message();
        assert!(!msg_ptr.is_null());

        // Use accessors that are guaranteed to succeed and never set
        // last-error. ad_check_permissions is NOT safe here — it can
        // return ErrPermDenied on macOS without Accessibility permission,
        // which overwrites the TLS error slot we're testing.
        for _ in 0..5 {
            let _ = ad_app_list_count(std::ptr::null());
            let _ = ad_window_list_count(std::ptr::null());
        }

        let after = ad_last_error_message();
        assert_eq!(msg_ptr, after);
        assert_eq!(ad_last_error_code(), rc);
    });
}
