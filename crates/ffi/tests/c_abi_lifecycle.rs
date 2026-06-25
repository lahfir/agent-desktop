mod common;

use common::{
    AdAppList, AdFindQuery, AdNativeHandle, AdResult, AdWaitArgs, AdWindowInfo, AdWindowList, CStr,
    ad_abi_version, ad_adapter_create, ad_adapter_create_with_session, ad_adapter_destroy,
    ad_app_list_count, ad_app_list_free, ad_app_list_get, ad_check_permissions, ad_execute_by_ref,
    ad_find, ad_free_handle, ad_free_string, ad_init, ad_last_error_code, ad_last_error_message,
    ad_list_apps, ad_list_windows, ad_set_log_callback, ad_snapshot, ad_status, ad_version,
    ad_wait, ad_window_list_count, ad_window_list_free, with_adapter,
};
use std::os::raw::c_char;
use std::sync::Mutex;

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
fn ad_version_returns_ok_with_valid_json_envelope() {
    unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_version(&mut out);
        assert_eq!(rc, AdResult::Ok, "ad_version must return OK");
        assert!(!out.is_null(), "out must be non-null on success");

        let json_str = CStr::from_ptr(out).to_string_lossy();
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("output must be valid JSON");

        assert_eq!(
            parsed["ok"].as_bool(),
            Some(true),
            "envelope ok must be true"
        );
        assert_eq!(
            parsed["command"].as_str(),
            Some("version"),
            "envelope command must be 'version'"
        );
        assert!(
            parsed["data"]["version"].is_string(),
            "data.version must be a string"
        );
        assert!(
            parsed["data"]["target"].is_string(),
            "data.target must be a string"
        );
        assert!(parsed["data"]["os"].is_string(), "data.os must be a string");

        let envelope_version = parsed["version"]
            .as_str()
            .expect("version field must exist");
        assert_eq!(
            envelope_version,
            agent_desktop_core::output::ENVELOPE_VERSION,
            "envelope version must match ENVELOPE_VERSION constant"
        );

        ad_free_string(out);
    }
}

#[test]
fn ad_version_null_out_returns_invalid_args() {
    unsafe {
        let rc = ad_version(std::ptr::null_mut());
        assert_eq!(
            rc,
            AdResult::ErrInvalidArgs,
            "null out must return ErrInvalidArgs"
        );
    }
}

#[test]
fn ad_version_success_preserves_prior_last_error() {
    unsafe {
        let rc_fail = ad_version(std::ptr::null_mut());
        assert_eq!(rc_fail, AdResult::ErrInvalidArgs);
        let err_before = ad_last_error_code();
        assert_eq!(err_before, AdResult::ErrInvalidArgs);

        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc_ok = ad_version(&mut out);
        assert_eq!(rc_ok, AdResult::Ok);

        assert_eq!(
            ad_last_error_code(),
            AdResult::ErrInvalidArgs,
            "success must not clear the prior last-error"
        );

        ad_free_string(out);
    }
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

#[test]
fn sessionless_adapter_has_no_session_id() {
    unsafe {
        let ptr = ad_adapter_create();
        assert!(!ptr.is_null(), "ad_adapter_create must not return null");
        let ctx = (*ptr)
            .command_context()
            .expect("command_context must succeed");
        assert_eq!(ctx.session_id(), None);
        ad_adapter_destroy(ptr);
    }
}

#[test]
fn session_adapter_carries_session_id() {
    unsafe {
        let session = std::ffi::CString::new("agent-a").unwrap();
        let ptr = ad_adapter_create_with_session(session.as_ptr());
        assert!(
            !ptr.is_null(),
            "ad_adapter_create_with_session must not return null"
        );
        let ctx = (*ptr)
            .command_context()
            .expect("command_context must succeed");
        assert_eq!(ctx.session_id(), Some("agent-a"));
        ad_adapter_destroy(ptr);
    }
}

#[test]
fn null_session_adapter_is_sessionless() {
    unsafe {
        let ptr = ad_adapter_create_with_session(std::ptr::null());
        assert!(
            !ptr.is_null(),
            "null session must yield a sessionless adapter"
        );
        let ctx = (*ptr)
            .command_context()
            .expect("command_context must succeed");
        assert_eq!(ctx.session_id(), None);
        ad_adapter_destroy(ptr);
    }
}

#[test]
fn invalid_utf8_session_returns_null_and_sets_invalid_args() {
    unsafe {
        let bad: [u8; 3] = [0xC3, 0xFF, 0x00];
        let ptr = ad_adapter_create_with_session(bad.as_ptr() as *const std::os::raw::c_char);
        assert!(ptr.is_null(), "invalid UTF-8 session must return null");
        assert_eq!(
            ad_last_error_code(),
            AdResult::ErrInvalidArgs,
            "invalid UTF-8 must set ErrInvalidArgs"
        );
        let msg = ad_last_error_message();
        assert!(
            !msg.is_null(),
            "error message must be set on invalid UTF-8 session"
        );
    }
}

#[test]
fn empty_session_returns_null_and_sets_invalid_args() {
    unsafe {
        let empty = std::ffi::CString::new("").unwrap();
        let ptr = ad_adapter_create_with_session(empty.as_ptr());
        assert!(ptr.is_null(), "empty session id must return null");
        assert_eq!(
            ad_last_error_code(),
            AdResult::ErrInvalidArgs,
            "empty session id must set ErrInvalidArgs"
        );
        let msg = ad_last_error_message();
        assert!(!msg.is_null(), "error message must be set on empty session");
    }
}

/// Captured delivery from the test callback.
struct Delivery {
    level: i32,
    message: String,
}

static RECORDER: Mutex<Vec<Delivery>> = Mutex::new(Vec::new());
static LOG_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Consumer-side recorder callback.  Must NOT emit tracing events (recursion).
unsafe extern "C" fn recorder_cb(level: i32, msg: *const c_char) {
    if msg.is_null() {
        return;
    }
    let message = unsafe { CStr::from_ptr(msg) }
        .to_string_lossy()
        .into_owned();
    if let Ok(mut guard) = RECORDER.lock() {
        guard.push(Delivery { level, message });
    }
}

fn clear_recorder() {
    if let Ok(mut g) = RECORDER.lock() {
        g.clear();
    }
}

fn drain_recorder() -> Vec<Delivery> {
    RECORDER
        .lock()
        .map(|mut g| g.drain(..).collect())
        .unwrap_or_default()
}

/// Register callback → emit a tracing event → callback receives level + non-null message.
#[test]
fn log_callback_register_delivers_event() {
    let _guard = LOG_TEST_LOCK.lock().unwrap();
    clear_recorder();

    unsafe {
        let rc = ad_set_log_callback(Some(recorder_cb));
        assert_eq!(rc, AdResult::Ok, "register must succeed");
    }

    tracing::error!(
        test_marker = "deliver_event",
        "log_callback_register_delivers_event"
    );

    let deliveries = drain_recorder();
    assert!(
        !deliveries.is_empty(),
        "at least one delivery expected after tracing::error!"
    );
    let d = deliveries
        .iter()
        .find(|d| d.message.contains("deliver_event"))
        .unwrap_or(&deliveries[0]);
    assert_eq!(d.level, 1, "ERROR maps to level 1");
    assert!(!d.message.is_empty(), "message must be non-empty");

    unsafe {
        let _ = ad_set_log_callback(None);
    }
}

/// A tracing event emitted from a spawned thread does not panic across the boundary.
#[test]
fn log_callback_spawned_thread_does_not_panic() {
    let _guard = LOG_TEST_LOCK.lock().unwrap();
    clear_recorder();

    unsafe {
        let rc = ad_set_log_callback(Some(recorder_cb));
        assert_eq!(rc, AdResult::Ok);
    }

    let handle = std::thread::spawn(|| {
        tracing::warn!(
            source = "spawned_thread",
            "log_callback_spawned_thread_does_not_panic"
        );
    });
    handle.join().expect("spawned thread must not panic");

    let deliveries = drain_recorder();
    assert!(
        !deliveries.is_empty(),
        "spawned-thread event must reach the callback"
    );

    unsafe {
        let _ = ad_set_log_callback(None);
    }
}

/// NULL unregisters the callback; subsequent events are not delivered.
#[test]
fn log_callback_null_unregisters() {
    let _guard = LOG_TEST_LOCK.lock().unwrap();
    clear_recorder();

    unsafe {
        let _ = ad_set_log_callback(Some(recorder_cb));
        let rc = ad_set_log_callback(None);
        assert_eq!(rc, AdResult::Ok, "NULL unregister must succeed");
    }
    clear_recorder();

    tracing::error!(test_marker = "after_null", "log_callback_null_unregisters");

    let deliveries = drain_recorder();
    assert!(
        deliveries.is_empty(),
        "no deliveries expected after NULL unregister, got {}",
        deliveries.len()
    );
}

/// A sensitive field (keyed by a SENSITIVE_KEYS name) is REDACTED in the
/// delivered message; non-sensitive fields are preserved.
#[test]
fn log_callback_redacts_sensitive_fields() {
    let _guard = LOG_TEST_LOCK.lock().unwrap();
    clear_recorder();

    unsafe {
        let rc = ad_set_log_callback(Some(recorder_cb));
        assert_eq!(rc, AdResult::Ok);
    }

    tracing::error!(
        password = "super_secret_password",
        token = "bearer_xyz",
        operation = "login_attempt",
        "log_callback_redacts_sensitive_fields"
    );

    let deliveries = drain_recorder();
    assert!(!deliveries.is_empty(), "expected at least one delivery");

    let combined: String = deliveries
        .iter()
        .map(|d| d.message.as_str())
        .collect::<Vec<_>>()
        .join(" ");

    assert!(
        !combined.contains("super_secret_password"),
        "raw password must not appear in callback message; got: {combined}"
    );
    assert!(
        !combined.contains("bearer_xyz"),
        "raw token must not appear in callback message; got: {combined}"
    );
    assert!(
        combined.contains("redacted"),
        "redaction marker must appear; got: {combined}"
    );
    assert!(
        combined.contains("login_attempt"),
        "non-sensitive field value must be preserved; got: {combined}"
    );

    unsafe {
        let _ = ad_set_log_callback(None);
    }
}

/// Re-registering a callback (and NULL then re-register) swaps the pointer
/// without returning an error.
#[test]
fn log_callback_re_register_is_ok() {
    let _guard = LOG_TEST_LOCK.lock().unwrap();
    clear_recorder();

    unsafe {
        assert_eq!(ad_set_log_callback(Some(recorder_cb)), AdResult::Ok);
        assert_eq!(ad_set_log_callback(None), AdResult::Ok);
        assert_eq!(ad_set_log_callback(Some(recorder_cb)), AdResult::Ok);
        assert_eq!(ad_set_log_callback(None), AdResult::Ok);
    }
}

#[test]
fn snapshot_null_out_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let rc = ad_snapshot(
            adapter,
            std::ptr::null(),
            0,
            6,
            false,
            false,
            std::ptr::null_mut(),
        );
        assert_eq!(
            rc,
            AdResult::ErrInvalidArgs,
            "null out is rejected by the outer guard before any adapter or thread check"
        );
    });
}

#[test]
fn snapshot_null_adapter_rejected() {
    unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(
            std::ptr::null(),
            std::ptr::null(),
            0,
            6,
            false,
            false,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null adapter must fail — got {rc:?} (ErrInternal on macOS off-main-thread is expected)"
        );
        assert!(out.is_null(), "out must stay null on null-adapter failure");
    }
}

#[test]
fn snapshot_invalid_utf8_app_rejected() {
    with_adapter(|adapter| unsafe {
        let bad: [u8; 3] = [0xC3, 0xFF, 0x00];
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(
            adapter,
            bad.as_ptr() as *const std::os::raw::c_char,
            0,
            6,
            false,
            false,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "invalid UTF-8 app must fail — got {rc:?}"
        );
        assert!(
            out.is_null(),
            "out must stay null on arg validation failure"
        );
    });
}

#[test]
fn snapshot_invalid_surface_rejected() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(adapter, std::ptr::null(), 99, 6, false, false, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "out-of-range surface must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on invalid surface");
    });
}

#[test]
fn snapshot_returns_a_result_code_and_frees_cleanly() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(adapter, std::ptr::null(), 0, 6, false, false, &mut out);
        let rc_i32 = rc as i32;
        assert!(
            rc_i32 <= 0,
            "ad_snapshot must return a valid AdResult (<=0), got {rc_i32}"
        );
        if !out.is_null() {
            let s = CStr::from_ptr(out).to_string_lossy();
            assert!(!s.is_empty(), "non-null out must be a non-empty string");
            ad_free_string(out);
        }
    });
}

#[test]
fn snapshot_out_contains_envelope_fields_on_any_response() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(adapter, std::ptr::null(), 0, 6, false, false, &mut out);
        if !out.is_null() {
            let s = CStr::from_ptr(out).to_string_lossy();
            assert!(
                s.contains("\"version\""),
                "envelope must carry 'version', got: {s}"
            );
            assert!(s.contains("\"ok\""), "envelope must carry 'ok', got: {s}");
            assert!(
                s.contains("\"command\""),
                "envelope must carry 'command', got: {s}"
            );
            assert!(
                s.contains("\"snapshot\""),
                "command value must be 'snapshot', got: {s}"
            );
            ad_free_string(out);
        } else {
            let rc_i32 = rc as i32;
            assert!(
                rc_i32 < 0,
                "null out is only valid on failure, got rc={rc_i32}"
            );
        }
    });
}

#[test]
fn status_null_adapter_returns_invalid_args() {
    unsafe {
        let mut out: *mut std::os::raw::c_char = 0xDEAD_BEEF as *mut std::os::raw::c_char;
        let rc = ad_status(std::ptr::null(), &mut out);
        assert_eq!(
            rc,
            AdResult::ErrInvalidArgs,
            "null adapter must return ErrInvalidArgs"
        );
        assert!(
            out.is_null(),
            "dirty out-param must be zeroed before early return on null adapter"
        );
    }
}

#[test]
fn status_null_out_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let rc = ad_status(adapter, std::ptr::null_mut());
        assert_eq!(
            rc,
            AdResult::ErrInvalidArgs,
            "null out must return ErrInvalidArgs"
        );
    });
}

#[test]
fn status_returns_ok_envelope_with_required_fields() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_status(adapter, &mut out);
        assert_eq!(rc, AdResult::Ok, "ad_status must return Ok");
        assert!(!out.is_null(), "out must be non-null on success");

        let json_str = CStr::from_ptr(out)
            .to_str()
            .expect("status output must be valid UTF-8");
        let val: serde_json::Value =
            serde_json::from_str(json_str).expect("status output must be valid JSON");

        assert_eq!(
            val["version"].as_str(),
            Some(agent_desktop_core::output::ENVELOPE_VERSION),
            "envelope version must match ENVELOPE_VERSION constant"
        );
        assert_eq!(val["ok"].as_bool(), Some(true), "ok must be true");
        assert_eq!(
            val["command"].as_str(),
            Some("status"),
            "command must be \"status\""
        );

        let data = &val["data"];
        assert!(
            data["platform"].is_string(),
            "data.platform must be present"
        );
        assert!(data["version"].is_string(), "data.version must be present");
        assert!(
            data["permissions"].is_object(),
            "data.permissions must be present"
        );

        ad_free_string(out);
    });
}

#[test]
fn status_free_string_cleans_up() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_status(adapter, &mut out);
        assert_eq!(rc, AdResult::Ok);
        assert!(!out.is_null());
        ad_free_string(out);
    });
}

#[test]
fn ad_wait_null_args_rejected() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_wait(adapter, std::ptr::null(), &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null args must be rejected, got {:?}",
            rc
        );
        assert!(out.is_null(), "out must stay null on null-args rejection");
        assert_eq!(
            ad_last_error_code(),
            rc,
            "last-error code must match returned AdResult (errno invariant)"
        );
    });
}

#[test]
fn ad_wait_null_out_rejected() {
    with_adapter(|adapter| unsafe {
        let args = AdWaitArgs {
            ms: 1,
            has_ms: true,
            element: std::ptr::null(),
            window: std::ptr::null(),
            text: std::ptr::null(),
            menu: false,
            menu_closed: false,
            notification: false,
            snapshot_id: std::ptr::null(),
            predicate: std::ptr::null(),
            value: std::ptr::null(),
            action: std::ptr::null(),
            count: 0,
            has_count: false,
            timeout_ms: 500,
            app: std::ptr::null(),
        };
        let rc = ad_wait(adapter, &args, std::ptr::null_mut());
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null out must be rejected, got {:?}",
            rc
        );
        assert_eq!(
            ad_last_error_code(),
            rc,
            "last-error code must match returned AdResult (errno invariant)"
        );
    });
}

#[test]
fn ad_wait_ms_mode_returns_ok_or_off_thread_error() {
    with_adapter(|adapter| unsafe {
        let args = AdWaitArgs {
            ms: 50,
            has_ms: true,
            element: std::ptr::null(),
            window: std::ptr::null(),
            text: std::ptr::null(),
            menu: false,
            menu_closed: false,
            notification: false,
            snapshot_id: std::ptr::null(),
            predicate: std::ptr::null(),
            value: std::ptr::null(),
            action: std::ptr::null(),
            count: 0,
            has_count: false,
            timeout_ms: 500,
            app: std::ptr::null(),
        };
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_wait(adapter, &args, &mut out);

        match rc {
            AdResult::Ok => {
                assert!(!out.is_null(), "Ok result must set out");
                let json_cstr = CStr::from_ptr(out);
                let json: serde_json::Value =
                    serde_json::from_str(json_cstr.to_str().unwrap()).unwrap();
                assert_eq!(json["ok"], serde_json::Value::Bool(true));
                assert_eq!(json["command"], "wait");
                ad_free_string(out);
            }
            AdResult::ErrInternal => {
                assert!(out.is_null(), "ErrInternal must leave out null");
                let msg = ad_last_error_message();
                assert!(!msg.is_null(), "error message must be set on failure");
                assert_eq!(
                    ad_last_error_code(),
                    AdResult::ErrInternal,
                    "last-error code must match returned AdResult (errno invariant)"
                );
            }
            other => panic!("unexpected result from ms-mode ad_wait: {:?}", other),
        }
    });
}

#[test]
fn execute_by_ref_null_out_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let action = common::default_action();
        let rc = ad_execute_by_ref(adapter, ref_id.as_ptr(), &action, 0, std::ptr::null_mut());
        assert_eq!(
            rc,
            AdResult::ErrInvalidArgs,
            "null out is rejected by the outer guard before any thread or adapter check"
        );
    });
}

#[test]
fn execute_by_ref_null_adapter_rejected() {
    unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let action = common::default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(std::ptr::null(), ref_id.as_ptr(), &action, 0, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null adapter must fail — got {rc:?} (ErrInternal on macOS off-main-thread is expected)"
        );
        assert!(out.is_null(), "out must stay null on failure");
    }
}

#[test]
fn execute_by_ref_null_ref_id_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let action = common::default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(adapter, std::ptr::null(), &action, 0, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null ref_id must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on null ref_id");
    });
}

#[test]
fn execute_by_ref_invalid_utf8_ref_id_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let bad: [u8; 3] = [0xC3, 0xFF, 0x00];
        let action = common::default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(
            adapter,
            bad.as_ptr() as *const std::os::raw::c_char,
            &action,
            0,
            &mut out,
        );
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "invalid UTF-8 ref_id must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on invalid UTF-8 ref_id");
    });
}

#[test]
fn execute_by_ref_null_action_rejected() {
    with_adapter(|adapter| unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(adapter, ref_id.as_ptr(), std::ptr::null(), 0, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "null action must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on null action");
    });
}

#[test]
fn execute_by_ref_invalid_ref_format_returns_invalid_args() {
    with_adapter(|adapter| unsafe {
        let bad_ref = std::ffi::CString::new("@e0").unwrap();
        let action = common::default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(adapter, bad_ref.as_ptr(), &action, 0, &mut out);
        assert!(
            matches!(rc, AdResult::ErrInvalidArgs | AdResult::ErrInternal),
            "bad ref format must fail — got {rc:?}"
        );
        assert!(out.is_null(), "out must stay null on bad ref format");
    });
}

#[test]
fn execute_by_ref_returns_error_envelope_when_no_refmap_exists() {
    with_adapter(|adapter| unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let action = common::default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(adapter, ref_id.as_ptr(), &action, 0, &mut out);
        let rc_i32 = rc as i32;
        assert!(
            rc_i32 <= 0,
            "must return a valid AdResult (<=0), got {rc_i32}"
        );
        if !out.is_null() {
            let s = CStr::from_ptr(out).to_string_lossy();
            assert!(
                s.contains("\"version\""),
                "envelope must carry 'version', got: {s}"
            );
            assert!(s.contains("\"ok\""), "envelope must carry 'ok', got: {s}");
            assert!(
                s.contains("\"command\""),
                "envelope must carry 'command', got: {s}"
            );
            ad_free_string(out);
        }
    });
}
