use super::*;

struct ProtocolTarget(LiveElement);

impl ObservationOps for ProtocolTarget {
    fn is_nonspatial_target(&self, _handle: &NativeHandle) -> bool {
        true
    }

    fn get_live_element(
        &self,
        _handle: &NativeHandle,
        _deadline: crate::Deadline,
    ) -> Result<LiveElement, AdapterError> {
        Ok(self.0.clone())
    }

    fn hit_test(
        &self,
        _handle: &NativeHandle,
        _point: crate::Point,
        _deadline: crate::Deadline,
    ) -> Result<crate::HitTestResult, AdapterError> {
        panic!("a nonspatial target cannot authorize pointer delivery")
    }
}

impl ActionOps for ProtocolTarget {}
impl InputOps for ProtocolTarget {}
impl SystemOps for ProtocolTarget {}

type LiveCorruption = fn(&mut LiveElement);

fn unsafe_live_states() -> [(LiveCorruption, crate::ErrorCode, &'static str); 6] {
    use crate::ErrorCode::{ActionFailed, PolicyDenied};
    [
        (
            |live| live.state.hidden = Some(true),
            ActionFailed,
            "visible (live hidden state is true)",
        ),
        (
            |live| live.state.offscreen = Some(true),
            ActionFailed,
            "visible (live offscreen state is true)",
        ),
        (
            |live| live.states_complete = false,
            ActionFailed,
            "visible (live offscreen state unavailable)",
        ),
        (
            |live| live.state.hidden = None,
            ActionFailed,
            "visible (bounds unavailable)",
        ),
        (
            |live| live.state.enabled = Some(false),
            ActionFailed,
            "enabled (live enabled state is false)",
        ),
        (
            |live| live.available_actions.clear(),
            PolicyDenied,
            "supported_action (direct semantic delivery is unavailable and physical fallback is denied)",
        ),
    ]
}

#[test]
fn nonspatial_semantic_targets_need_live_state_but_not_screen_bounds() {
    let (entry, adapter) = target("menuitem", &[capability::CLICK, capability::RIGHT_CLICK]);
    let mut live = adapter.0;
    live.bounds = None;
    let check = |live: LiveElement, request: ActionRequest| {
        check_live(
            &entry,
            &NativeHandle::null(),
            &ProtocolTarget(live),
            &request,
        )
    };
    for action in [Action::Click, Action::RightClick] {
        let report = check(live.clone(), ActionRequest::headless(action.clone()))
            .expect("protocol actions have no screen position");
        assert!(report.actionable);
        assert_eq!(report.pointer_delivery, PointerDelivery::Semantic);
        assert_eq!(report.verified_point, None);
        assert_eq!(report.presentation_point, None);
        for (corrupt, code, refusal) in unsafe_live_states() {
            let mut unsafe_live = live.clone();
            corrupt(&mut unsafe_live);
            let error = check(unsafe_live, ActionRequest::headless(action.clone())).unwrap_err();
            assert_eq!(error.code, code, "{refusal}");
            assert!(error.message.contains(refusal), "{}", error.message);
            assert_eq!(error.disposition, crate::DeliverySemantics::not_delivered());
        }
        let headed = check(live.clone(), ActionRequest::headed(action)).unwrap_err();
        assert_eq!(headed.code, crate::ErrorCode::ActionFailed);
        assert!(
            headed
                .message
                .contains("visible (bounds unavailable), stable (live bounds unavailable)"),
            "{}",
            headed.message
        );
    }
    let hover = check(live, ActionRequest::headless(Action::Hover)).unwrap_err();
    assert_eq!(hover.code, crate::ErrorCode::PolicyDenied);
    assert!(
        hover
            .message
            .contains("policy (action requires cursor movement but policy denies it)"),
        "{}",
        hover.message
    );
}

struct ClassifiedTarget(ProtocolTarget, Result<bool, AdapterError>);

impl ObservationOps for ClassifiedTarget {
    fn is_nonspatial_target_with_deadline(
        &self,
        _: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<bool, AdapterError> {
        deadline.ensure_remaining()?;
        self.1.clone()
    }

    fn get_live_element(
        &self,
        handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<LiveElement, AdapterError> {
        self.0.get_live_element(handle, deadline)
    }
}
impl ActionOps for ClassifiedTarget {}
impl InputOps for ClassifiedTarget {}
impl SystemOps for ClassifiedTarget {}

#[test]
fn semantic_geometry_exemption_requires_successful_native_provenance() {
    let (entry, adapter) = target("menuitem", &[capability::CLICK]);
    let mut live = adapter.0;
    live.bounds = None;
    live.state.hidden = Some(false);
    live.state.offscreen = None;
    for classified in [true, false] {
        let adapter = ClassifiedTarget(ProtocolTarget(live.clone()), Ok(classified));
        let result = check_live(
            &entry,
            &NativeHandle::null(),
            &adapter,
            &ActionRequest::headless(Action::Click),
        );
        assert_eq!(result.is_ok(), classified);
    }
    for code in [
        crate::ErrorCode::PermDenied,
        crate::ErrorCode::Timeout,
        crate::ErrorCode::StaleRef,
    ] {
        let adapter = ClassifiedTarget(
            ProtocolTarget(live.clone()),
            Err(AdapterError::new(
                code.clone(),
                "Native provenance read failed",
            )),
        );
        let error = check_live(
            &entry,
            &NativeHandle::null(),
            &adapter,
            &ActionRequest::headless(Action::Click),
        )
        .unwrap_err();
        assert_eq!(error.code, code);
    }
}
