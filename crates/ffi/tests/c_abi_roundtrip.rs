/// Integration tests for the observe→act loop at the C boundary.
///
/// # Test structure
///
/// - `stale_ref_returns_ok_false_error_envelope` — always runs in CI.
///   Sets a temp HOME (empty refmap), calls `ad_execute_by_ref`, and asserts
///   that the returned envelope has `ok:false` with both `error.code` (string)
///   and `error.message` (string), and that `command` equals `"execute_by_ref"`.
///   On macOS the main-thread guard fires before the command path is reached
///   (libtest runs bodies off the main thread), returning `ErrInternal` with a
///   null `*out` — that path is tolerated.
///
/// - `snapshot_execute_by_ref_live_roundtrip` — marked `#[ignore]`.
///   The full observe→act loop (real `ad_snapshot` → `@e` ref →
///   `ad_execute_by_ref` against a live app) cannot run under the default
///   libtest harness on macOS: libtest schedules test bodies on worker threads,
///   and the AX/main-thread guard blocks every AX call made off the main
///   thread.  CI proof of the full loop is tracked as the deferred
///   external-consumer smoke harness (Python ctypes) — plan unit U9 / Phase B.
///   To run the roundtrip manually:
///   ```text
///   cargo test -p agent-desktop-ffi --tests c_abi_roundtrip \
///       snapshot_execute_by_ref_live_roundtrip -- --ignored
///   ```
///   Run from a process that owns the main thread (e.g. the E2E harness).
///   AX accessibility permission must be granted and a target app must be open.
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
/// `ok:false` and carry both `error.code` (string) and `error.message`
/// (string) — the latter guaranteed by the CLAUDE.md error contract — proving
/// the unified error-envelope contract holds at the ABI boundary.  The exact
/// `error.code` value is not pinned: an empty refmap with an `@e1` ref may
/// surface either `SNAPSHOT_NOT_FOUND` or `STALE_REF` depending on the load
/// path; pinning either would make this test flaky.
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
            val["error"]["message"].is_string(),
            "error.message must be a string; got: {s}"
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
/// This test is `#[ignore]` by design and must remain so in headless CI.
///
/// **Why ignored**: libtest schedules test bodies on worker threads.  On macOS
/// every AX API call (including `ad_snapshot` and `ad_execute_by_ref`) must
/// originate on the main thread; the AX guard returns `ErrInternal` immediately
/// when called off it.  There is no libtest API to pin a test to the main
/// thread, so the full observe→act loop cannot be verified inside a libtest
/// integration test on macOS.
///
/// **Deferred CI proof**: the full-loop proof — real `ad_snapshot` producing
/// `@e` refs consumed by `ad_execute_by_ref` against a live app at the C
/// boundary — is tracked as plan unit U9 / Phase B: an external-consumer smoke
/// harness (Python ctypes) that runs in the E2E environment where the process
/// itself owns the main thread.
///
/// **Manual execution** (requires AX permission + a running target app):
/// ```text
/// cargo test -p agent-desktop-ffi --tests c_abi_roundtrip \
///     snapshot_execute_by_ref_live_roundtrip -- --ignored
/// ```
/// Run from a process that owns the main thread (e.g. the E2E harness).  Do
/// NOT un-ignore this test — it will fail in any headless CI that uses libtest.
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
