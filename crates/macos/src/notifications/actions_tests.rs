use agent_desktop_core::{AdapterError, Deadline, ErrorCode};

#[test]
fn hard_ax_error_falls_through_to_the_next_dismiss_strategy() {
    let deadline = Deadline::standard().expect("deadline");
    let outcomes = [
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "AXDismiss failed",
        )),
        Ok(true),
    ];
    let mut attempts = 0;
    let mut dismissed = false;

    for outcome in outcomes {
        attempts += 1;
        if super::strategy_succeeded(outcome, deadline).expect("strategy result") {
            dismissed = true;
            break;
        }
    }

    assert!(dismissed);
    assert_eq!(attempts, 2);
}

#[test]
fn permission_denial_is_terminal_for_dismiss_strategies() {
    let result = super::strategy_succeeded(
        Err(AdapterError::permission_denied()),
        Deadline::standard().expect("deadline"),
    );

    assert_eq!(
        result.expect_err("permission denial must abort").code,
        ErrorCode::PermDenied
    );
}

#[test]
fn expired_deadline_is_terminal_for_dismiss_strategies() {
    let deadline = Deadline::after(0).expect("deadline");
    let result = super::strategy_succeeded(
        Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "AXDismiss failed",
        )),
        deadline,
    );

    assert_eq!(
        result.expect_err("expired deadline must abort").code,
        ErrorCode::Timeout
    );
}
