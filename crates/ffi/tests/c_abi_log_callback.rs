mod common;

use common::{AdResult, CStr, ad_set_log_callback};
use std::os::raw::c_char;
use std::sync::Mutex;

struct Delivery {
    level: i32,
    message: String,
}

static RECORDER: Mutex<Vec<Delivery>> = Mutex::new(Vec::new());
static LOG_TEST_LOCK: Mutex<()> = Mutex::new(());

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
