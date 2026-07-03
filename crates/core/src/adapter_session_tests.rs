use super::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

struct FlagSession {
    closed: Arc<AtomicBool>,
}

impl AdapterSession for FlagSession {
    fn close(self: Box<Self>) -> Result<(), AdapterError> {
        self.closed.store(true, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn boxed_adapter_session_close_runs_through_dyn_dispatch() {
    let closed = Arc::new(AtomicBool::new(false));
    let session: Box<dyn AdapterSession> = Box::new(FlagSession {
        closed: closed.clone(),
    });

    session.close().unwrap();

    assert!(
        closed.load(Ordering::SeqCst),
        "close() must run through Box<dyn AdapterSession> dispatch, proving the trait is object-safe"
    );
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn boxed_adapter_session_is_send_and_sync() {
    assert_send_sync::<Box<dyn AdapterSession>>();
}
