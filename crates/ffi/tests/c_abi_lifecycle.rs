mod common;

use agent_desktop_core::NativeHandle;
use common::{
    AdExactSurfaceList, AdExactWindowList, AdFindQuery, AdNativeHandle, AdResult, AdWindowInfo,
    AdWindowList, CStr, ad_adapter_create, ad_adapter_destroy, ad_app_list_count, ad_app_list_free,
    ad_app_list_get, ad_check_permissions, ad_exact_surface_list_count, ad_exact_surface_list_free,
    ad_exact_surface_list_get, ad_exact_window_list_count, ad_exact_window_list_free,
    ad_exact_window_list_get, ad_find, ad_free_handle, ad_last_error_code, ad_last_error_message,
    ad_list_apps, ad_list_surfaces_exact, ad_list_windows, ad_list_windows_exact,
    ad_window_list_count, ad_window_list_free, with_adapter,
};
use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

struct DropProbe(Arc<AtomicU32>);

impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn null_adapter_rejected_without_ub() {
    unsafe {
        let mut list = std::ptr::null_mut();
        let rc = ad_list_apps(std::ptr::null(), &mut list);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert!(list.is_null(), "out-param must stay null on failure");

        let rc2 = ad_check_permissions(std::ptr::null());
        assert_eq!(rc2, AdResult::ErrInvalidArgs);
    }
}

#[test]
fn null_out_param_rejected_before_write() {
    with_adapter(|adapter| unsafe {
        let rc = ad_list_apps(adapter, std::ptr::null_mut());
        assert_eq!(rc, AdResult::ErrInvalidArgs);
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

        assert_eq!(ad_exact_window_list_count(std::ptr::null()), 0);
        assert!(ad_exact_window_list_get(std::ptr::null(), 0).is_null());
        ad_exact_window_list_free(std::ptr::null_mut());

        assert_eq!(ad_exact_surface_list_count(std::ptr::null()), 0);
        assert!(ad_exact_surface_list_get(std::ptr::null(), 0).is_null());
        ad_exact_surface_list_free(std::ptr::null_mut());
    }
}

#[test]
fn dirty_out_param_is_cleared_before_early_return_on_worker_thread() {
    with_adapter(|adapter| unsafe {
        let fake_ptr = 0xDEAD_BEEF as *mut common::AdAppList;
        let mut list = fake_ptr;
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
        let mut list = std::ptr::null_mut();
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
fn exact_list_out_params_are_zeroed_on_stub_or_platform_failure() {
    with_adapter(|adapter| unsafe {
        let mut windows = std::ptr::dangling_mut::<AdExactWindowList>();
        let windows_rc = ad_list_windows_exact(adapter, std::ptr::null(), true, &mut windows);
        if windows_rc == AdResult::Ok {
            assert!(!windows.is_null());
            ad_exact_window_list_free(windows);
        } else {
            assert!(windows.is_null());
        }

        let mut surfaces = std::ptr::dangling_mut::<AdExactSurfaceList>();
        let surfaces_rc = ad_list_surfaces_exact(adapter, 1, &mut surfaces);
        if surfaces_rc == AdResult::Ok {
            assert!(!surfaces.is_null());
            ad_exact_surface_list_free(surfaces);
        } else {
            assert!(surfaces.is_null());
        }
    });
}

#[test]
fn find_returns_not_found_on_empty_query_against_no_window() {
    with_adapter(|adapter| unsafe {
        let bad_win: AdWindowInfo = std::mem::zeroed();
        let mut query: AdFindQuery = std::mem::zeroed();
        query.control.version = agent_desktop_ffi::AD_FIND_QUERY_VERSION;
        let mut handle = AdNativeHandle {
            ptr: std::ptr::null(),
        };
        let rc = ad_find(adapter, &bad_win, &query, &mut handle);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
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

#[test]
fn free_handle_rejects_forged_allocation_pointer_without_dereferencing_it() {
    with_adapter(|adapter| unsafe {
        let drops = Arc::new(AtomicU32::new(0));
        let native = Box::new(NativeHandle::new(DropProbe(Arc::clone(&drops))));
        let raw = Box::into_raw(native);
        let mut handle = AdNativeHandle { ptr: raw.cast() };

        let rc = ad_free_handle(adapter, &mut handle);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
        assert_eq!(handle.ptr, raw.cast());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(Box::from_raw(raw));
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    });
}

#[test]
fn last_error_survives_successful_calls() {
    unsafe {
        let adapter = ad_adapter_create();
        assert!(!adapter.is_null());
        let mut out: AdWindowInfo = std::mem::zeroed();
        let rc = common::ad_launch_app(adapter, std::ptr::null(), 0, &mut out);
        assert_eq!(rc, AdResult::ErrInvalidArgs);
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
