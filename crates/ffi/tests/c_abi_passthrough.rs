/// Stub-adapter passthrough tests — gate with `--features stub-adapter`.
///
/// Every adapter-touching `ad_*` entrypoint is called against the stub
/// adapter. The stub's `PlatformAdapter` impl delegates all methods to the
/// trait defaults, which uniformly return `AdapterError::not_supported(…)` →
/// `ErrorCode::PlatformNotSupported`. The JSON envelope therefore carries
/// `"ok":false` and `"error":{"code":"PLATFORM_NOT_SUPPORTED","suggestion":…}`.
///
/// Exception — `ad_check_permissions`:
/// The stub's `permission_report()` returns `PermissionState::Denied` (the
/// trait default), not `Unknown`. The FFI maps `Denied` to `ErrPermDenied
/// (-1)`, not `ErrPlatformNotSupported (-8)`. This is the documented signal
/// that permissions are unavailable on the platform; callers should treat
/// both `ErrPermDenied` and `ErrPlatformNotSupported` as "adapter not
/// operational here".
///
/// Main-thread tolerance:
/// `ad_snapshot`, `ad_wait`, and `ad_execute_by_ref` each call
/// `require_main_thread()` before touching the adapter. The libtest harness
/// spawns each `#[test]` on a worker thread, so on macOS those guards fire
/// first and return `ErrInternal` with no JSON envelope. The tests accept
/// either outcome:
///   - `ErrInternal` (off-main-thread on macOS, no envelope produced) — ok.
///   - `ErrPlatformNotSupported` + valid JSON envelope — ok.
///
/// Both outcomes are correct; neither is a regression.
///
/// Commands gated by `#[cfg(feature = "stub-adapter")]` so they compile only
/// when the feature is active. The normal test build (`cargo test -p
/// agent-desktop-ffi --tests`) never compiles or runs this file.
#[cfg(feature = "stub-adapter")]
mod common;

#[cfg(feature = "stub-adapter")]
use common::{
    AdResult, AdWaitArgs, CStr, ad_adapter_create, ad_adapter_destroy, ad_check_permissions,
    ad_execute_by_ref, ad_free_string, ad_last_error_code, ad_last_error_message, ad_snapshot,
    ad_status, ad_version, ad_wait, default_action, with_adapter,
};

/// A helper that parses the JSON envelope written to `*out` and asserts
/// `PLATFORM_NOT_SUPPORTED` shape. Returns `true` when the envelope was
/// present and verified; `false` when `*out` is null (meaning the function
/// returned early — e.g. due to the macOS off-main-thread guard firing before
/// the adapter was reached).
#[cfg(feature = "stub-adapter")]
unsafe fn assert_platform_not_supported_envelope(out: *mut std::os::raw::c_char) -> bool {
    if out.is_null() {
        return false;
    }
    let json_str = unsafe { CStr::from_ptr(out) }
        .to_str()
        .expect("envelope must be valid UTF-8");
    let parsed: serde_json::Value =
        serde_json::from_str(json_str).expect("envelope must be valid JSON");
    assert_eq!(
        parsed["ok"].as_bool(),
        Some(false),
        "stub adapter must produce ok:false envelope — got: {json_str}"
    );
    assert_eq!(
        parsed["error"]["code"].as_str(),
        Some("PLATFORM_NOT_SUPPORTED"),
        "error.code must be PLATFORM_NOT_SUPPORTED — got: {json_str}"
    );
    let suggestion = parsed["error"]["suggestion"].as_str().unwrap_or_default();
    assert!(
        !suggestion.is_empty(),
        "error.suggestion must be non-empty — got: {json_str}"
    );
    true
}

/// `ad_version` has no adapter dependency and must always succeed even on a
/// stub build.
#[cfg(feature = "stub-adapter")]
#[test]
fn stub_ad_version_always_succeeds() {
    unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_version(&mut out);
        assert_eq!(
            rc,
            AdResult::Ok,
            "ad_version must succeed regardless of adapter (no adapter dependency)"
        );
        assert!(!out.is_null(), "out must be non-null on success");
        let json_str = CStr::from_ptr(out).to_str().expect("valid UTF-8");
        let parsed: serde_json::Value = serde_json::from_str(json_str).expect("valid JSON");
        assert_eq!(parsed["ok"].as_bool(), Some(true));
        assert!(parsed["data"]["version"].is_string());
        ad_free_string(out);
    }
}

/// `ad_check_permissions` maps the stub adapter's `Denied` permission state
/// to `ErrPermDenied (-1)`, not `ErrPlatformNotSupported (-8)`. This is the
/// documented exception. Cross-platform callers must treat both codes as
/// "adapter not operational here".
#[cfg(feature = "stub-adapter")]
#[test]
fn stub_ad_check_permissions_returns_err_perm_denied() {
    with_adapter(|adapter| unsafe {
        let rc = ad_check_permissions(adapter);
        assert_eq!(
            rc,
            AdResult::ErrPermDenied,
            "stub adapter permission_report() returns Denied → ErrPermDenied (-1), \
             not ErrPlatformNotSupported (-8). Both mean the adapter is not operational."
        );
        let msg = ad_last_error_message();
        assert!(
            !msg.is_null(),
            "last-error message must be set on ErrPermDenied"
        );
        assert_eq!(ad_last_error_code(), AdResult::ErrPermDenied);
    });
}

/// `ad_status` is not main-thread gated. Under the stub adapter it returns a
/// valid JSON envelope with `ok:true` because `execute_with_report_with_context`
/// reports the Denied permission state as a valid (non-error) status payload.
///
/// This test asserts the envelope is produced and is valid JSON — the specific
/// permission values are stub-specific but the shape must always match the CLI.
#[cfg(feature = "stub-adapter")]
#[test]
fn stub_ad_status_returns_valid_envelope() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_status(adapter, &mut out);
        assert!(
            !out.is_null(),
            "ad_status must always produce an envelope (ok or error) — rc={rc:?}"
        );
        let json_str = CStr::from_ptr(out)
            .to_str()
            .expect("envelope must be valid UTF-8");
        let parsed: serde_json::Value =
            serde_json::from_str(json_str).expect("envelope must be valid JSON");
        assert!(
            parsed["ok"].is_boolean(),
            "envelope must carry ok field — got: {json_str}"
        );
        assert_eq!(
            parsed["command"].as_str(),
            Some("status"),
            "command field must be 'status'"
        );
        let _ = rc;
        ad_free_string(out);
    });
}

/// `ad_snapshot` calls `require_main_thread()` before reaching the adapter.
/// On macOS the libtest worker thread fires the guard first, returning
/// `ErrInternal` with no envelope. On non-macOS the call proceeds to the stub
/// adapter and must produce a `PLATFORM_NOT_SUPPORTED` error envelope.
#[cfg(feature = "stub-adapter")]
#[test]
fn stub_ad_snapshot_platform_not_supported_or_off_main_thread() {
    with_adapter(|adapter| unsafe {
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(adapter, std::ptr::null(), 0, 6, false, false, &mut out);
        match rc {
            AdResult::ErrInternal => {
                assert!(
                    out.is_null(),
                    "ErrInternal (off-main-thread guard) must leave out null"
                );
            }
            AdResult::ErrPlatformNotSupported => {
                let had_envelope = assert_platform_not_supported_envelope(out);
                assert!(
                    had_envelope,
                    "ErrPlatformNotSupported must be accompanied by an error envelope"
                );
                ad_free_string(out);
            }
            other => {
                panic!(
                    "stub ad_snapshot must return ErrInternal (macOS off-main-thread) or \
                     ErrPlatformNotSupported, got {other:?}"
                );
            }
        }
    });
}

/// `ad_wait` calls `require_main_thread()` before reaching the adapter.
/// Same tolerance as `ad_snapshot`: accepts `ErrInternal` (off-main-thread
/// on macOS) or `ErrPlatformNotSupported` with a valid envelope.
#[cfg(feature = "stub-adapter")]
#[test]
fn stub_ad_wait_platform_not_supported_or_off_main_thread() {
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
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_wait(adapter, &args, &mut out);
        match rc {
            AdResult::ErrInternal => {
                assert!(
                    out.is_null(),
                    "ErrInternal (off-main-thread guard) must leave out null"
                );
            }
            AdResult::ErrPlatformNotSupported => {
                let had_envelope = assert_platform_not_supported_envelope(out);
                assert!(
                    had_envelope,
                    "ErrPlatformNotSupported must be accompanied by an error envelope"
                );
                ad_free_string(out);
            }
            other => {
                panic!(
                    "stub ad_wait must return ErrInternal (macOS off-main-thread) or \
                     ErrPlatformNotSupported, got {other:?}"
                );
            }
        }
    });
}

/// `ad_execute_by_ref` calls `require_main_thread()` before reaching the
/// adapter. Same main-thread tolerance applies. Uses a syntactically valid
/// ref-id so arg-decode succeeds before the thread check.
#[cfg(feature = "stub-adapter")]
#[test]
fn stub_ad_execute_by_ref_platform_not_supported_or_off_main_thread() {
    with_adapter(|adapter| unsafe {
        let ref_id = std::ffi::CString::new("@e1").unwrap();
        let action = default_action();
        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_execute_by_ref(
            adapter,
            ref_id.as_ptr(),
            std::ptr::null(),
            &action,
            0,
            &mut out,
        );
        match rc {
            AdResult::ErrInternal => {
                assert!(
                    out.is_null(),
                    "ErrInternal (off-main-thread guard) must leave out null"
                );
            }
            AdResult::ErrPlatformNotSupported
            | AdResult::ErrSnapshotNotFound
            | AdResult::ErrStaleRef => {
                if !out.is_null() {
                    let json_str = CStr::from_ptr(out)
                        .to_str()
                        .expect("envelope must be valid UTF-8");
                    let parsed: serde_json::Value =
                        serde_json::from_str(json_str).expect("envelope must be valid JSON");
                    assert_eq!(parsed["ok"].as_bool(), Some(false));
                    assert!(!parsed["error"]["code"].as_str().unwrap_or("").is_empty());
                    ad_free_string(out);
                }
            }
            other => {
                panic!(
                    "stub ad_execute_by_ref must return ErrInternal, ErrPlatformNotSupported, \
                     ErrSnapshotNotFound, or ErrStaleRef, got {other:?}"
                );
            }
        }
    });
}

/// Confirm that `ad_adapter_create` itself does not panic under the stub
/// feature and produces a non-null handle that can be destroyed cleanly.
#[cfg(feature = "stub-adapter")]
#[test]
fn stub_adapter_create_and_destroy_round_trip() {
    unsafe {
        let adapter = ad_adapter_create();
        assert!(
            !adapter.is_null(),
            "stub ad_adapter_create must not return null"
        );
        ad_adapter_destroy(adapter);
    }
}
