/// Integration tests for the observe→act loop at the C boundary.
///
/// # Test structure
///
/// - `stale_ref_returns_ok_false_error_envelope` — guaranteed to run in CI.
///   Sets a temp HOME (empty refmap), calls `ad_execute_by_ref`, and asserts
///   that the returned envelope has `ok:false` with an `error.code` field.
///   Because `ad_execute_by_ref` has a main-thread guard on macOS (the libtest
///   harness runs bodies off the main thread), `ErrInternal` with a null `*out`
///   is also a valid outcome and is tolerated.
///
/// - `snapshot_execute_by_ref_live_roundtrip` — marked `#[ignore]`.
///   Requires a real running app, AX accessibility permission, and execution on
///   the main thread (e.g. via the E2E harness or by running with
///   `cargo test -p agent-desktop-ffi --tests c_abi_roundtrip
///    snapshot_execute_by_ref_live_roundtrip -- --ignored`
///   from a process that IS the main thread, i.e. not the default libtest runner).
mod common;

use common::{AdResult, CStr, ad_execute_by_ref, ad_free_string, ad_snapshot, default_action};
use std::sync::Mutex;

static HOME_LOCK: Mutex<()> = Mutex::new(());

struct TestHome {
    _lock: std::sync::MutexGuard<'static, ()>,
    dir: std::path::PathBuf,
    prev: Option<std::ffi::OsString>,
}

impl TestHome {
    fn new() -> Self {
        let lock = HOME_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "agent-desktop-ffi-roundtrip-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let prev = std::env::var_os("HOME");
        unsafe { std::env::set_var("HOME", &dir) };
        Self {
            _lock: lock,
            dir,
            prev,
        }
    }
}

impl Drop for TestHome {
    fn drop(&mut self) {
        match self.prev.as_ref() {
            Some(prev) => unsafe { std::env::set_var("HOME", prev) },
            None => unsafe { std::env::remove_var("HOME") },
        }
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Verifies the error-envelope contract at the C boundary when no refmap exists.
///
/// Sets a temp HOME so the refmap store is empty. Calls `ad_execute_by_ref`
/// with a well-formed `@e1` ref. On macOS the main-thread guard fires first
/// (libtest runs off the main thread), returning `ErrInternal` with `out`
/// remaining null — that is tolerated. When the command path does execute
/// (non-macOS or future main-thread execution), the envelope must have
/// `ok:false` and an `error` object, proving the unified error-envelope
/// contract holds at the ABI boundary.
#[test]
fn stale_ref_returns_ok_false_error_envelope() {
    let _home = TestHome::new();

    with_adapter_raw(|adapter| unsafe {
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

        let rc_i32 = rc as i32;
        assert!(
            rc_i32 <= 0,
            "must return a valid AdResult (<=0), got {rc_i32}"
        );

        if out.is_null() {
            assert_eq!(
                rc,
                AdResult::ErrInternal,
                "null *out is only expected when ErrInternal (main-thread guard on macOS), got {rc:?}"
            );
            return;
        }

        let s = CStr::from_ptr(out).to_string_lossy();
        let val: serde_json::Value = serde_json::from_str(&s).expect("response must be valid JSON");

        assert_eq!(
            val["ok"].as_bool(),
            Some(false),
            "error envelope ok must be false; got: {s}"
        );
        assert!(
            val["error"].is_object(),
            "error envelope must carry an error object; got: {s}"
        );
        assert!(
            val["error"]["code"].is_string(),
            "error.code must be a string; got: {s}"
        );
        assert!(
            val["version"].is_string(),
            "envelope version must be present; got: {s}"
        );
        assert_eq!(
            val["command"].as_str(),
            Some("execute_by_ref"),
            "command must be 'execute_by_ref'; got: {s}"
        );

        ad_free_string(out);
    });
}

/// Live snapshot→ref→execute_by_ref roundtrip at the C boundary.
///
/// This test MUST be ignored by default: it requires a real running
/// application with an accessible UI tree, macOS Accessibility permission
/// granted for the test process, and execution on the main thread (not the
/// libtest off-main-thread runner).
///
/// To run manually once AX permission is granted and a target app is running:
/// ```
/// cargo test -p agent-desktop-ffi --tests c_abi_roundtrip \
///     snapshot_execute_by_ref_live_roundtrip -- --ignored
/// ```
/// Even then, the test body must somehow reach the main thread (e.g. via
/// the E2E harness or a custom test runner that schedules tests on the main
/// runloop thread).
#[test]
#[ignore = "requires AX permission, a live app, and main-thread execution — run via E2E harness"]
fn snapshot_execute_by_ref_live_roundtrip() {
    let _home = TestHome::new();

    with_adapter_raw(|adapter| unsafe {
        let mut snap_out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_snapshot(adapter, std::ptr::null(), 0, 6, true, false, &mut snap_out);

        assert_eq!(rc, AdResult::Ok, "snapshot must succeed");
        assert!(
            !snap_out.is_null(),
            "snapshot out must be non-null on success"
        );

        let snap_str = CStr::from_ptr(snap_out).to_string_lossy();
        let snap_val: serde_json::Value =
            serde_json::from_str(&snap_str).expect("snapshot must be valid JSON");

        assert_eq!(
            snap_val["ok"].as_bool(),
            Some(true),
            "snapshot envelope ok must be true"
        );

        let ref_id = find_first_ref_in_tree(&snap_val)
            .expect("snapshot must contain at least one @e ref in the tree");

        ad_free_string(snap_out);

        let ref_cstr = std::ffi::CString::new(ref_id.as_str()).unwrap();
        let action = default_action();
        let mut exec_out: *mut std::os::raw::c_char = std::ptr::null_mut();

        let exec_rc = ad_execute_by_ref(
            adapter,
            ref_cstr.as_ptr(),
            std::ptr::null(),
            &action,
            0,
            &mut exec_out,
        );

        assert!(
            !exec_out.is_null(),
            "execute_by_ref must produce an envelope (ok or error), got rc={exec_rc:?}"
        );

        let exec_str = CStr::from_ptr(exec_out).to_string_lossy();
        let exec_val: serde_json::Value =
            serde_json::from_str(&exec_str).expect("execute_by_ref response must be valid JSON");

        assert!(
            exec_val["version"].is_string(),
            "response must carry envelope version; got: {exec_str}"
        );
        assert!(
            exec_val["ok"].is_boolean(),
            "response must carry ok field; got: {exec_str}"
        );
        assert_eq!(
            exec_val["command"].as_str(),
            Some("execute_by_ref"),
            "command must be execute_by_ref; got: {exec_str}"
        );

        ad_free_string(exec_out);
    });
}

fn find_first_ref_in_tree(snap: &serde_json::Value) -> Option<String> {
    search_refs(snap)
}

fn search_refs(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::Object(map) => {
            if let Some(r) = map.get("ref") {
                if let Some(s) = r.as_str() {
                    if s.starts_with("@e") {
                        return Some(s.to_owned());
                    }
                }
            }
            for v in map.values() {
                if let Some(found) = search_refs(v) {
                    return Some(found);
                }
            }
            None
        }
        serde_json::Value::Array(arr) => {
            for v in arr {
                if let Some(found) = search_refs(v) {
                    return Some(found);
                }
            }
            None
        }
        _ => None,
    }
}

fn with_adapter_raw<F: FnOnce(*mut common::AdAdapter)>(body: F) {
    unsafe {
        let adapter = common::ad_adapter_create();
        assert!(!adapter.is_null(), "ad_adapter_create must not return null");
        body(adapter);
        common::ad_adapter_destroy(adapter);
    }
}
