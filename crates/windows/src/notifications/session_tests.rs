use agent_desktop_core::{AdapterError, ErrorCode, InteractionPolicy};

use super::{closed_center_policy_error, merge_session_result};

#[test]
fn a_closed_center_is_policy_denied_headlessly() {
    let error = closed_center_policy_error(InteractionPolicy::headless());

    assert_eq!(error.code, ErrorCode::PolicyDenied);
    assert!(
        error
            .suggestion
            .as_deref()
            .is_some_and(|value| value.contains("--headed"))
    );
}

#[test]
fn the_operation_error_wins_when_cleanup_also_fails() {
    let operation = AdapterError::new(ErrorCode::ElementNotFound, "operation failed");
    let cleanup = AdapterError::timeout("cleanup failed");

    let error = merge_session_result::<()>(Err(operation), Err(cleanup)).unwrap_err();

    assert_eq!(error.code, ErrorCode::ElementNotFound);
}

#[test]
fn a_cleanup_failure_replaces_an_apparent_operation_success() {
    let cleanup = AdapterError::timeout("cleanup failed");

    let error = merge_session_result(Ok("value"), Err(cleanup)).unwrap_err();

    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn a_clean_cleanup_passes_the_operation_through() {
    let value = merge_session_result(Ok(41), Ok(())).expect("the value survives");

    assert_eq!(value, 41);
}

#[cfg(target_os = "windows")]
mod live {
    use agent_desktop_core::{Deadline, InteractionPolicy, SnapshotSurface};

    use crate::notifications::list::list_notifications;
    use crate::system::shell_surface::resolve_surface;
    use crate::system::shell_surface_open::{close_surface, open_surface};
    use crate::system::test_support::SHELL_SURFACE_LOCK;

    fn deadline(ms: u64) -> Deadline {
        Deadline::after(ms).expect("deadline")
    }

    fn headed() -> InteractionPolicy {
        InteractionPolicy::headed()
    }

    fn center_is_open() -> bool {
        resolve_surface(SnapshotSurface::ActionCenter, deadline(10_000))
            .expect("the desktop is readable")
            .is_some()
    }

    #[test]
    fn a_session_on_an_already_open_center_leaves_it_open() {
        crate::tree::fixture::bootstrap();
        let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = close_surface(SnapshotSurface::ActionCenter, deadline(8_000));
        open_surface(SnapshotSurface::ActionCenter, headed(), deadline(15_000))
            .expect("the center opens for the pre-arrangement");

        let listed = list_notifications(&Default::default(), headed(), deadline(20_000))
            .expect("an already-present center needs no raise, so the listing proceeds");

        assert!(
            center_is_open(),
            "the session must restore the state it found: an open center stays open"
        );
        let _ = listed;
        let _ = close_surface(SnapshotSurface::ActionCenter, deadline(8_000));
    }

    #[test]
    fn a_session_on_a_closed_center_leaves_it_closed() {
        crate::tree::fixture::bootstrap();
        let _lock = SHELL_SURFACE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = close_surface(SnapshotSurface::ActionCenter, deadline(8_000));

        list_notifications(&Default::default(), headed(), deadline(20_000))
            .expect("the listing itself succeeds against the closed-then-raised center");

        assert!(
            !center_is_open(),
            "the session must restore the state it found: a closed center ends closed"
        );
    }
}
