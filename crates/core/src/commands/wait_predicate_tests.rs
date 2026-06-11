use super::test_support::{
    PredicateAdapter, snapshot_with_disabled_ref, snapshot_with_one_ref, wait_for_element_test,
};
use super::*;
use crate::{
    adapter::{NativeHandle, PlatformAdapter},
    commands::wait_predicate,
    element_state::ElementState,
    error::AdapterError,
    refs::RefEntry,
    refs_test_support::HomeGuard,
};
use std::sync::Mutex;

struct FlippingPredicateAdapter {
    states: Mutex<Vec<Vec<String>>>,
}

impl PlatformAdapter for FlippingPredicateAdapter {
    fn resolve_element_strict_with_timeout(
        &self,
        _entry: &RefEntry,
        _timeout: std::time::Duration,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    fn get_live_state(&self, _handle: &NativeHandle) -> Result<Option<ElementState>, AdapterError> {
        let states = self.states.lock().unwrap().pop().unwrap_or_default();
        Ok(Some(ElementState {
            role: "button".into(),
            states,
            value: None,
        }))
    }
}

#[test]
fn element_wait_enabled_predicate_uses_live_state() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = PredicateAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec![],
            value: None,
        }),
        value: None,
        bounds: None,
    };

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Enabled,
        50,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "enabled");
    assert_eq!(value["observed"]["enabled"], true);
}

#[test]
fn element_wait_value_predicate_matches_live_value_without_leaking_it() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = PredicateAdapter {
        state: None,
        value: Some("ready".into()),
        bounds: None,
    };

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Value("ready".into()),
        1,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "value");
    assert_eq!(value["observed"]["matched"], true);
    assert_eq!(value["observed"]["value_chars"], 5);
    assert!(value["observed"].get("value").is_none());
}

#[test]
fn element_wait_timeout_reports_last_actionability_observation() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_disabled_ref();
    let adapter = PredicateAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec!["disabled".into()],
            value: None,
        }),
        value: None,
        bounds: None,
    };

    let err = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Actionable(
            crate::action_request::ActionRequest::headless(crate::action::Action::Click),
        ),
        50,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    match err {
        AppError::Adapter(adapter_error) => {
            let details = adapter_error.details.unwrap();
            assert_eq!(details["predicate"], "actionable");
            assert_eq!(details["last_observed"]["actionable"], false);
        }
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn element_wait_actionable_uses_live_state() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_disabled_ref();
    let adapter = PredicateAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec![],
            value: None,
        }),
        value: None,
        bounds: None,
    };

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Actionable(
            crate::action_request::ActionRequest::headless(crate::action::Action::Click),
        ),
        1,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "actionable");
    assert_eq!(value["observed"]["actionable"], true);
}

#[test]
fn element_wait_actionable_retries_until_live_state_converges() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_disabled_ref();
    let adapter = FlippingPredicateAdapter {
        states: Mutex::new(vec![vec![], vec!["disabled".into()]]),
    };

    let value = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Actionable(
            crate::action_request::ActionRequest::headless(crate::action::Action::Click),
        ),
        250,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["predicate"], "actionable");
    assert_eq!(value["observed"]["actionable"], true);
}

#[test]
fn element_wait_actionable_type_fails_on_uneditable_role() {
    let _guard = HomeGuard::new();
    let snapshot_id = snapshot_with_one_ref();
    let adapter = PredicateAdapter {
        state: Some(ElementState {
            role: "button".into(),
            states: vec![],
            value: None,
        }),
        value: None,
        bounds: None,
    };

    let err = wait_for_element_test(
        "@e1".into(),
        Some(snapshot_id),
        wait_predicate::ElementPredicate::Actionable(
            crate::action_request::ActionRequest::focus_fallback(crate::action::Action::TypeText(
                String::new(),
            )),
        ),
        50,
        &adapter,
        &crate::context::CommandContext::default(),
    )
    .unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    match err {
        AppError::Adapter(adapter_error) => {
            let details = adapter_error.details.unwrap();
            assert_eq!(details["last_observed"]["actionable"], false);
        }
        _ => panic!("expected adapter error"),
    }
}

#[test]
fn wait_action_flag_is_rejected_outside_actionable_predicate() {
    let err =
        wait_predicate::ElementPredicate::parse(Some("enabled"), None, Some("type")).unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn wait_actionable_rejects_unknown_action() {
    let err = wait_predicate::ElementPredicate::parse(Some("actionable"), None, Some("hover"))
        .unwrap_err();

    assert_eq!(err.code(), "INVALID_ARGS");
}

#[test]
fn actionable_parse_mirrors_each_real_command_policy() {
    use crate::interaction_policy::InteractionPolicy;

    let request_for = |name: Option<&str>| match wait_predicate::ElementPredicate::parse(
        Some("actionable"),
        None,
        name,
    )
    .unwrap()
    {
        wait_predicate::ElementPredicate::Actionable(request) => request,
        other => panic!("expected actionable predicate, got {other:?}"),
    };

    assert_eq!(request_for(None).policy, InteractionPolicy::headless());
    assert_eq!(
        request_for(Some("click")).policy,
        InteractionPolicy::headless()
    );
    assert_eq!(
        request_for(Some("type")).policy,
        InteractionPolicy::focus_fallback()
    );
    assert_eq!(
        request_for(Some("set-value")).policy,
        InteractionPolicy::headless()
    );
    assert_eq!(
        request_for(Some("clear")).policy,
        InteractionPolicy::headless()
    );
}
