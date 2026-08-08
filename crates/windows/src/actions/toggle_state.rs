//! Toggle / Check / Uncheck via TogglePattern with Invoke fallback (A19-2).

use agent_desktop_core::{ActionStep, AdapterError, Deadline, ErrorCode, InteractionPolicy};
use std::time::{Duration, Instant};

use crate::actions::chain::{
    ALREADY_LABEL, ChainDef, ChainRung, DeliveryOutcome, INVOKE_LABEL, build_step,
    capped_verification_end, execute_chain,
};
use crate::actions::post_state::after_delivery;
use crate::tree::element::UIAElement;

pub(crate) const TOGGLE_LABEL: &str = "TogglePattern.Toggle";

const TOGGLE_TIMEOUT: Duration = Duration::from_millis(600);
const TOGGLE_STABLE: Duration = Duration::from_millis(200);
const POLL_SLICE: Duration = Duration::from_millis(25);

pub(crate) const TOGGLE_CHAIN: ChainDef = ChainDef {
    suggestion: "Refresh the snapshot and retry, or target an element that advertises Toggle or Invoke.",
    continue_after_unverified_delivery: false,
};

const CHECK_SUGGESTION: &str =
    "Refresh the snapshot and inspect the checked state before deciding whether to retry.";

/// UIA `ToggleState` values (A15-7 / A19-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToggleKind {
    Off,
    On,
    Indeterminate,
}

impl ToggleKind {
    pub(crate) fn from_i32(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Off),
            1 => Some(Self::On),
            2 => Some(Self::Indeterminate),
            _ => None,
        }
    }

    fn matches_checked(self, want_checked: bool) -> bool {
        matches!((self, want_checked), (Self::On, true) | (Self::Off, false))
    }
}

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        ALREADY_LABEL, ActionStep, AdapterError, CHECK_SUGGESTION, ChainRung, Deadline,
        DeliveryOutcome, ErrorCode, INVOKE_LABEL, Instant, InteractionPolicy, POLL_SLICE,
        TOGGLE_CHAIN, TOGGLE_LABEL, TOGGLE_STABLE, TOGGLE_TIMEOUT, ToggleKind, UIAElement,
        after_delivery, build_step, capped_verification_end, execute_chain,
    };
    use crate::actions::mutation::{classify_success, classify_write};
    use crate::actions::post_state::delivery_occurred;
    use crate::system::permissions::ensure_budget;
    use crate::tree::properties::read_one;
    use crate::tree::property_ids::TreeProperty;
    use uiautomation::patterns::{UIInvokePattern, UITogglePattern};

    pub(crate) fn toggle_steps(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let toggle_ok = toggle_available(element);
        let before = if toggle_ok {
            read_toggle_state(element)
        } else {
            None
        };
        let invoke_ok = invoke_available(element);
        toggle_judged_for(
            deadline,
            policy,
            toggle_ok,
            invoke_ok,
            || delivered_with_observe(before, deadline, element, pattern_toggle),
            || delivered_with_observe(before, deadline, element, pattern_invoke),
        )
    }

    pub(crate) fn check_steps(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        check_uncheck_steps(element, true, policy, deadline)
    }

    pub(crate) fn uncheck_steps(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        check_uncheck_steps(element, false, policy, deadline)
    }

    fn check_uncheck_steps(
        element: &UIAElement,
        want_checked: bool,
        _policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        check_uncheck_judged_for(
            deadline,
            want_checked,
            toggle_available(element),
            invoke_available(element),
            || read_toggle_kind(element),
            || pattern_toggle(element),
            || pattern_invoke(element),
        )
    }

    /// Injected Toggle chain — unit-test seam and live path.
    pub(crate) fn toggle_judged_for(
        deadline: Deadline,
        policy: InteractionPolicy,
        toggle_ok: bool,
        invoke_ok: bool,
        mut toggle: impl FnMut() -> Result<DeliveryOutcome, AdapterError>,
        mut invoke: impl FnMut() -> Result<DeliveryOutcome, AdapterError>,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let mut toggle_run = || {
            if !toggle_ok {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            toggle()
        };
        let mut invoke_run = || {
            if !invoke_ok {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            invoke()
        };
        execute_chain(
            deadline,
            &TOGGLE_CHAIN,
            policy,
            &mut [
                ChainRung {
                    label: TOGGLE_LABEL,
                    requires_headed: false,
                    run: &mut toggle_run,
                },
                ChainRung {
                    label: INVOKE_LABEL,
                    requires_headed: false,
                    run: &mut invoke_run,
                },
            ],
        )
    }

    /// Injected Check/Uncheck — unit-test seam and live path.
    pub(crate) fn check_uncheck_judged_for(
        deadline: Deadline,
        want_checked: bool,
        toggle_ok: bool,
        invoke_ok: bool,
        mut read_state: impl FnMut() -> Option<ToggleKind>,
        mut toggle: impl FnMut() -> Result<bool, AdapterError>,
        mut invoke: impl FnMut() -> Result<bool, AdapterError>,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        ensure_budget(deadline)?;
        let before = read_state();
        if before.is_some_and(|state| state.matches_checked(want_checked)) {
            return Ok(vec![build_step(
                ALREADY_LABEL,
                DeliveryOutcome::SatisfiedNoDelivery,
            )]);
        }
        let mut steps = Vec::new();
        if toggle_ok {
            for attempt in 0..2 {
                ensure_budget(deadline)?;
                if !toggle()? {
                    break;
                }
                let last = attempt == 1;
                let verified = poll_checked(
                    want_checked,
                    deadline,
                    &mut read_state,
                    if last { None } else { before },
                )
                .map_err(after_delivery)?;
                steps.push(build_step(
                    TOGGLE_LABEL,
                    DeliveryOutcome::from_delivery(true, verified),
                ));
                if verified {
                    return Ok(steps);
                }
            }
        }
        if invoke_ok && !delivery_occurred(&steps) {
            ensure_budget(deadline)?;
            if invoke()? {
                let verified = poll_checked(want_checked, deadline, &mut read_state, None)
                    .map_err(after_delivery)?;
                steps.push(build_step(
                    INVOKE_LABEL,
                    DeliveryOutcome::from_delivery(true, verified),
                ));
                return Ok(steps);
            }
            steps.push(build_step(INVOKE_LABEL, DeliveryOutcome::NotDelivered));
        }
        if delivery_occurred(&steps) {
            return Ok(steps);
        }
        Err(exhausted())
    }

    fn delivered_with_observe(
        before: Option<ToggleKind>,
        deadline: Deadline,
        element: &UIAElement,
        deliver: impl FnOnce(&UIAElement) -> Result<bool, AdapterError>,
    ) -> Result<DeliveryOutcome, AdapterError> {
        if !deliver(element)? {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        Ok(DeliveryOutcome::from_delivery(
            true,
            observe_change(before, deadline, element).map_err(after_delivery)?,
        ))
    }

    fn toggle_available(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::ToggleAvailable).flag() == Some(true)
    }

    fn invoke_available(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::InvokeAvailable).flag() == Some(true)
    }

    fn read_toggle_state(element: &UIAElement) -> Option<ToggleKind> {
        read_one(element, TreeProperty::ToggleState)
            .number()
            .and_then(ToggleKind::from_i32)
    }

    fn read_toggle_kind(element: &UIAElement) -> Option<ToggleKind> {
        if !toggle_available(element) {
            return None;
        }
        read_toggle_state(element)
    }

    fn pattern_toggle(element: &UIAElement) -> Result<bool, AdapterError> {
        match element.0.get_pattern::<UITogglePattern>() {
            Ok(pattern) => match pattern.toggle() {
                Ok(()) => classify_success(),
                Err(error) => classify_write("Toggle", TOGGLE_LABEL, &error),
            },
            Err(error) => classify_write("get_pattern", TOGGLE_LABEL, &error),
        }
    }

    fn pattern_invoke(element: &UIAElement) -> Result<bool, AdapterError> {
        match element.0.get_pattern::<UIInvokePattern>() {
            Ok(pattern) => match pattern.invoke() {
                Ok(()) => classify_success(),
                Err(error) => classify_write("Invoke", INVOKE_LABEL, &error),
            },
            Err(error) => classify_write("get_pattern", INVOKE_LABEL, &error),
        }
    }

    fn observe_change(
        before: Option<ToggleKind>,
        deadline: Deadline,
        element: &UIAElement,
    ) -> Result<bool, AdapterError> {
        let Some(before) = before else {
            return Ok(false);
        };
        let end = capped_verification_end(deadline, TOGGLE_TIMEOUT)?;
        let mut candidate: Option<(ToggleKind, Instant)> = None;
        loop {
            if let Some(current) = read_toggle_kind(element) {
                if current != before {
                    match &mut candidate {
                        Some((value, since)) if *value == current => {
                            if since.elapsed() >= TOGGLE_STABLE {
                                return Ok(true);
                            }
                        }
                        _ => candidate = Some((current, Instant::now())),
                    }
                } else {
                    candidate = None;
                }
            }
            if Instant::now() >= end {
                return Ok(false);
            }
            sleep_poll(deadline)?;
        }
    }

    fn poll_checked(
        want_checked: bool,
        deadline: Deadline,
        read_state: &mut impl FnMut() -> Option<ToggleKind>,
        early_exit_after_change_from: Option<ToggleKind>,
    ) -> Result<bool, AdapterError> {
        let end = capped_verification_end(deadline, TOGGLE_TIMEOUT)?;
        loop {
            match read_state() {
                Some(state) if state.matches_checked(want_checked) => return Ok(true),
                Some(state)
                    if early_exit_after_change_from.is_some_and(|before| state != before) =>
                {
                    sleep_poll(deadline)?;
                    return Ok(
                        read_state().is_some_and(|state| state.matches_checked(want_checked))
                    );
                }
                _ => {}
            }
            if Instant::now() >= end {
                return Ok(false);
            }
            sleep_poll(deadline)?;
        }
    }

    fn sleep_poll(deadline: Deadline) -> Result<(), AdapterError> {
        std::thread::sleep(deadline.remaining_slice(POLL_SLICE)?);
        Ok(())
    }

    fn exhausted() -> AdapterError {
        AdapterError::new(ErrorCode::ActionFailed, "All chain steps exhausted")
            .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
            .with_suggestion(CHECK_SUGGESTION)
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionStep, AdapterError, Deadline, InteractionPolicy, UIAElement};

    pub(crate) fn toggle_steps(
        _: &UIAElement,
        _: InteractionPolicy,
        _: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Toggle"))
    }
    pub(crate) fn check_steps(
        _: &UIAElement,
        _: InteractionPolicy,
        _: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Check"))
    }
    pub(crate) fn uncheck_steps(
        _: &UIAElement,
        _: InteractionPolicy,
        _: Deadline,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        Err(AdapterError::not_supported("Uncheck"))
    }
}

pub(crate) use imp::{check_steps, toggle_steps, uncheck_steps};

#[cfg(all(test, target_os = "windows"))]
pub(crate) use imp::{check_uncheck_judged_for, toggle_judged_for};

#[cfg(all(test, target_os = "windows"))]
#[path = "toggle_state_tests.rs"]
mod tests;
