mod common;

use common::{
    AdAppList, AdFindQuery, AdNativeHandle, AdResult, AdWindowInfo, AdWindowList, CStr,
    ad_abi_version, ad_adapter_create, ad_adapter_destroy, ad_app_list_count, ad_app_list_free,
    ad_app_list_get, ad_check_permissions, ad_find, ad_free_handle, ad_init, ad_last_error_code,
    ad_last_error_message, ad_list_apps, ad_list_windows, ad_window_list_count,
    ad_window_list_free, with_adapter,
};

#[test]
fn abi_version_matches_rust_constant() {
    unsafe {
        assert_eq!(
            ad_abi_version(),
            agent_desktop_ffi::AD_ABI_VERSION_MAJOR,
            "ad_abi_version() must equal AD_ABI_VERSION_MAJOR"
        );
    }
}

#[test]
fn ad_init_succeeds_with_current_major() {
    unsafe {
        assert_eq!(
            ad_init(agent_desktop_ffi::AD_ABI_VERSION_MAJOR),
            AdResult::Ok
        );
    }
}

#[test]
fn ad_init_rejects_future_major_and_sets_last_error() {
    unsafe {
        let rc = ad_init(agent_desktop_ffi::AD_ABI_VERSION_MAJOR + 1);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        let msg = ad_last_error_message();
        assert!(
            !msg.is_null(),
            "last-error message must be non-null after mismatch"
        );
        let _ = CStr::from_ptr(msg).to_string_lossy();
        assert_eq!(ad_last_error_code(), AdResult::ErrInvalidArgs);
    }
}

#[test]
fn ad_init_rejects_zero_major_and_sets_last_error() {
    unsafe {
        let rc = ad_init(0);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        let msg = ad_last_error_message();
        assert!(
            !msg.is_null(),
            "last-error message must be non-null after zero-major mismatch"
        );
        let _ = CStr::from_ptr(msg).to_string_lossy();
        assert_eq!(ad_last_error_code(), AdResult::ErrInvalidArgs);
    }
}

#[test]
fn null_adapter_rejected_without_ub() {
    unsafe {
        let mut list: *mut AdAppList = std::ptr::null_mut();
        let rc = ad_list_apps(std::ptr::null(), &mut list);
        assert!(matches!(
            rc,
            AdResult::ErrInvalidArgs | AdResult::ErrInternal
        ));
        assert!(list.is_null(), "out-param must stay null on failure");

        let rc2 = ad_check_permissions(std::ptr::null());
        assert_eq!(rc2, AdResult::ErrInvalidArgs);
    }
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
fn dirty_out_param_is_cleared_before_early_return_on_worker_thread() {
    with_adapter(|adapter| unsafe {
        let fake_ptr = 0xDEAD_BEEF as *mut AdAppList;
        let mut list: *mut AdAppList = fake_ptr;
        let rc = ad_list_apps(adapter, &mut list);
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
fn list_handle_lifecycle_roundtrip() {
    with_adapter(|adapter| unsafe {
        let mut list: *mut AdAppList = std::ptr::null_mut();
        let rc = ad_list_apps(adapter, &mut list);
        if rc == AdResult::Ok {
            assert!(!list.is_null());
            let count = ad_app_list_count(list);
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
fn last_error_survives_successful_calls() {
    unsafe {
        let adapter = ad_adapter_create();
        assert!(!adapter.is_null());
        let mut out: AdWindowInfo = std::mem::zeroed();
        let rc = common::ad_launch_app(adapter, std::ptr::null(), 0, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "must fail, got {:?}",
            rc
        );
        let msg_ptr = ad_last_error_message();
        assert!(!msg_ptr.is_null());

        for _ in 0..5 {
            let _ = ad_app_list_count(std::ptr::null());
            let _ = ad_window_list_count(std::ptr::null());
        }

        let after = ad_last_error_message();
        assert_eq!(msg_ptr, after);
        assert_eq!(ad_last_error_code(), rc);
        ad_adapter_destroy(adapter);
    }
}
