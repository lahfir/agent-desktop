mod common;

use agent_desktop_core::session::{
    SessionTraceMode, StartSessionOptions, start_session, trace_dir,
};
use common::{
    AdResult, CStr, ad_adapter_create_with_session, ad_adapter_destroy, ad_check_permissions,
    ad_free_string, ad_status,
};
use std::ffi::CString;
use std::fs;
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
            "agent-desktop-ffi-session-trace-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
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
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn trace_dir_for(session_id: &str) -> std::path::PathBuf {
    trace_dir(session_id).unwrap()
}

#[test]
fn ffi_trace_on_session_writes_segment() {
    let _home = TestHome::new();
    let manifest = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();
    for call in 0..2 {
        unsafe {
            let session = CString::new(manifest.id.as_str()).unwrap();
            let ptr = ad_adapter_create_with_session(session.as_ptr());
            assert!(!ptr.is_null());
            let ctx = (*ptr)
                .command_context()
                .expect("command_context must succeed");
            ctx.trace("ffi.event", serde_json::json!({ "call": call }))
                .unwrap();
            ad_adapter_destroy(ptr);
        }
    }
    let trace_dir = trace_dir_for(&manifest.id);
    let segments: Vec<_> = fs::read_dir(trace_dir)
        .unwrap()
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    assert_eq!(
        segments.len(),
        1,
        "a long-lived process must write one segment across many FFI calls"
    );
    let body = fs::read_to_string(segments[0].path()).unwrap();
    assert!(body.contains("\"call\":0"));
    assert!(body.contains("\"call\":1"));
}

#[test]
fn ffi_plain_session_writes_no_trace_files() {
    let _home = TestHome::new();
    unsafe {
        let session = CString::new("plain-session").unwrap();
        let ptr = ad_adapter_create_with_session(session.as_ptr());
        assert!(!ptr.is_null());
        let ctx = (*ptr)
            .command_context()
            .expect("command_context must succeed");
        ctx.trace("ffi.event", serde_json::json!({})).unwrap();
        ad_check_permissions(ptr);
        ad_adapter_destroy(ptr);
    }
    assert!(!trace_dir_for("plain-session").exists());
}

/// R8 claims `command.start`/`command.end` boundary events fire over FFI via
/// the generated entrypoints. The two tests above only prove the trace
/// plumbing itself works by hand-crafting a `CommandContext::trace` call —
/// they never go through a real `ad_*` entrypoint's codegen-injected
/// `context.command_scope(...)` / `scope.complete(...)` pair.
///
/// This test drives the real `ad_status` entrypoint end-to-end (chosen
/// because it needs no accessibility permission and no main-thread affinity,
/// so it runs headless in CI — see `crates/ffi/src/commands/generated.rs`)
/// under a trace-enabled session, then reads the on-disk trace segment and
/// asserts both boundary events were actually written by the generated code,
/// in the right order.
#[test]
fn ffi_real_ad_status_entrypoint_emits_command_start_and_end_trace_events() {
    let _home = TestHome::new();
    let manifest = start_session(StartSessionOptions {
        name: None,
        trace: SessionTraceMode::On,
        force: false,
        ..Default::default()
    })
    .unwrap();

    unsafe {
        let session = CString::new(manifest.id.as_str()).unwrap();
        let ptr = ad_adapter_create_with_session(session.as_ptr());
        assert!(!ptr.is_null());

        let mut out: *mut std::os::raw::c_char = std::ptr::null_mut();
        let rc = ad_status(ptr, &mut out);
        assert_eq!(
            rc,
            AdResult::Ok,
            "the real ad_status entrypoint must succeed under a trace-enabled session"
        );
        assert!(
            !out.is_null(),
            "ad_status must produce an envelope on success"
        );
        let body = CStr::from_ptr(out).to_string_lossy().into_owned();
        assert!(
            body.contains("\"command\":\"status\""),
            "envelope command must be 'status', got: {body}"
        );
        ad_free_string(out);
        ad_adapter_destroy(ptr);
    }

    let trace_dir = trace_dir_for(&manifest.id);
    let segments: Vec<_> = fs::read_dir(trace_dir)
        .unwrap()
        .flatten()
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    assert_eq!(
        segments.len(),
        1,
        "a single FFI call must write exactly one trace segment"
    );

    let trace_body = fs::read_to_string(segments[0].path()).unwrap();
    let events: Vec<serde_json::Value> = trace_body
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();

    let start_idx = events.iter().position(|event| {
        event["event"].as_str() == Some("command.start")
            && event["command"].as_str() == Some("status")
    });
    let end_idx = events.iter().position(|event| {
        event["event"].as_str() == Some("command.end")
            && event["command"].as_str() == Some("status")
            && event["ok"].as_bool() == Some(true)
    });

    assert!(
        start_idx.is_some(),
        "trace segment must contain a command.start event fired by ad_status's real \
         command_scope(\"status\") call, got: {trace_body}"
    );
    assert!(
        end_idx.is_some(),
        "trace segment must contain a command.end event fired by ad_status's real \
         scope.complete(...) call, got: {trace_body}"
    );
    assert!(
        start_idx < end_idx,
        "command.start must be recorded before command.end for the same invocation"
    );
}
