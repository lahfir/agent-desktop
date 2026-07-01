mod common;

use agent_desktop_core::session::{
    SessionTraceMode, StartSessionOptions, start_session, trace_dir,
};
use common::{ad_adapter_create_with_session, ad_adapter_destroy, ad_check_permissions};
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
