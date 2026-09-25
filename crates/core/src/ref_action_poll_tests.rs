use super::*;

#[test]
fn preflight_preserves_explicit_non_retryable_failures() {
    for code in [
        ErrorCode::ActionFailed,
        ErrorCode::AppUnresponsive,
        ErrorCode::Timeout,
    ] {
        let mut state = RefActionPollState::default();
        let error = AdapterError::new(code.clone(), "terminal native read")
            .with_details(json!({ "retryable": false, "native_error": -25202 }));
        let result =
            handle_actionability_failure(&mut state, error, Deadline::after(5_000).unwrap());
        let error = result.expect_err("an explicit stop must not become another poll");
        assert_eq!(error.code, code);
        assert_eq!(error.details.unwrap()["native_error"], -25202);
        assert!(state.last_report.is_none());
    }
}

#[test]
fn preflight_identity_failures_are_terminal() {
    assert!(is_permanent_actionability_error(&ErrorCode::StaleRef));
    assert!(is_permanent_actionability_error(
        &ErrorCode::AmbiguousTarget
    ));
    assert!(!is_permanent_error(&ErrorCode::StaleRef));
}

#[test]
fn timeout_returns_a_settled_stale_ref_with_elapsed() {
    let mut state = RefActionPollState::default();
    state.record_resolve_error(
        &AdapterError::stale_ref_because("Stored ref does not match any live element")
            .with_details(json!({
                "kind": "resolve_no_candidate",
                "complete": true,
                "retryable": true,
            })),
    );

    let error = timeout(&state, Deadline::after(1).expect("deadline"));

    assert_eq!(error.code, ErrorCode::StaleRef);
    let details = error.details.expect("settled details");
    assert_eq!(details["kind"], "resolve_no_candidate");
    assert!(details.get("elapsed_ms").is_some());
}

#[test]
fn timeout_returns_a_settled_ambiguous_target() {
    let mut state = RefActionPollState::default();
    state.record_resolve_error(
        &AdapterError::ambiguous_target("Multiple live elements match the stored identity")
            .with_details(json!({
                "kind": "resolve_ambiguous",
                "complete": true,
                "retryable": false,
            })),
    );

    let error = timeout(&state, Deadline::after(1).expect("deadline"));

    assert_eq!(error.code, ErrorCode::AmbiguousTarget);
}

#[test]
fn timeout_keeps_timeout_when_the_resolve_search_was_incomplete() {
    let mut state = RefActionPollState::default();
    state.record_resolve_error(
        &AdapterError::stale_ref_because("Strict resolution slice expired").with_details(json!({
            "kind": "resolve_no_candidate",
            "complete": false,
            "retryable": true,
        })),
    );

    let error = timeout(&state, Deadline::after(1).expect("deadline"));

    assert_eq!(error.code, ErrorCode::Timeout);
    assert_eq!(
        error.details.expect("timeout details")["kind"],
        "actionability_timeout"
    );
}

#[test]
fn timeout_prefers_an_earlier_settled_stale_ref_over_a_later_cutoff() {
    let mut state = RefActionPollState::default();
    state.record_resolve_error(
        &AdapterError::stale_ref_because("Stored ref does not match any live element")
            .with_details(json!({
                "kind": "resolve_no_candidate",
                "complete": true,
                "retryable": true,
            })),
    );
    state.record_resolve_error(
        &AdapterError::timeout("Operation exceeded its deadline")
            .with_details(json!({ "kind": "deadline" })),
    );

    let error = timeout(&state, Deadline::after(1).expect("deadline"));

    assert_eq!(error.code, ErrorCode::StaleRef);
}

#[test]
fn timeout_forgets_the_settled_stale_ref_once_an_attempt_resolves() {
    let mut state = RefActionPollState::default();
    state.record_resolve_error(
        &AdapterError::stale_ref_because("Stored ref does not match any live element")
            .with_details(json!({
                "kind": "resolve_no_candidate",
                "complete": true,
                "retryable": true,
            })),
    );
    state.note_resolved();

    let error = timeout(&state, Deadline::after(1).expect("deadline"));

    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn timeout_preserves_transient_ambiguity_evidence() {
    let state = RefActionPollState {
        saw_ambiguity: true,
        ..Default::default()
    };
    let error = timeout(&state, Deadline::after(1).expect("deadline"));

    assert_eq!(
        error.details.expect("timeout details")["transient_ambiguity"],
        true
    );
}

#[test]
fn incomplete_live_reads_report_the_tool_limitation() {
    let mut state = RefActionPollState::default();
    state.record_preflight_error(
        &AdapterError::new(ErrorCode::AppUnresponsive, "incomplete").with_details(json!({
            "kind": "live_element_evidence",
            "complete": false,
            "query_stats": {
                "reads": { "native_read_failures": 2 },
                "traversal": { "nodes_visited": 1 },
            },
        })),
    );

    let error = timeout(&state, Deadline::after(1).expect("deadline"));
    assert_eq!(error.code, ErrorCode::ActionFailed);
    let details = error.details.expect("structured details");
    assert_eq!(details["kind"], "live_read_incomplete");
    assert_eq!(details["native_read_failures"], 2);
    assert_eq!(details["nodes_visited"], 1);
    assert!(
        !error
            .suggestion
            .expect("recovery suggestion")
            .contains("busy or unresponsive")
    );
}
