//! `execute_action` match over every `Action` variant.
//!
//! Click routes through the Invoke + Legacy chain. SetValue / Clear route
//! through `value_write` with post-state attachment. Toggle / Check / Uncheck
//! and Expand / Collapse attach post-state after delivery. Select and Scroll
//! route through their dedicated modules. SetFocus routes through `focus`.
//! ScrollTo runs the ScrollIntoView spine plus the ancestor ladder when
//! ScrollItem is absent or leaves geometry unchanged. Capabilities that need
//! key synthesis or physical multi-click fail `PLATFORM_NOT_SUPPORTED`
//! naming the missing machinery.

use agent_desktop_core::{
    Action, ActionResult, ActionStep, AdapterError, Deadline, Direction, ErrorCode,
    InteractionLease, InteractionPolicy, NativeHandle, action_request::ActionRequest,
};

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        Action, ActionRequest, ActionResult, ActionStep, AdapterError, Deadline, Direction,
        ErrorCode, InteractionLease, InteractionPolicy, NativeHandle,
    };
    use crate::actions::chain::{
        CLICK_CHAIN, ChainRung, DeliveryOutcome, build_step, execute_chain,
    };
    use crate::actions::disclosure::{collapse_steps, expand_steps};
    use crate::actions::focus::focus_element;
    use crate::actions::mutation::{classify_mutation, classify_success};
    use crate::actions::post_state::post_state_for_steps;
    use crate::actions::scroll::scroll_steps;
    use crate::actions::scroll_into_view::scroll_into_view_outcome;
    use crate::actions::select::select_steps;
    use crate::actions::toggle_state::{check_steps, toggle_steps, uncheck_steps};
    use crate::actions::value_write::{clear_steps, set_value_steps};
    use crate::system::permissions::ensure_budget;
    use crate::tree::automation::{ERR_NONE, UiaFailure, failure_of};
    use crate::tree::element::{UIAElement, uia_element};
    use crate::tree::live_read::corroborate_verified_process;
    use crate::tree::properties::read_one;
    use crate::tree::property_ids::TreeProperty;
    use agent_desktop_core::LocatorField;
    use uiautomation::patterns::{UIInvokePattern, UILegacyIAccessiblePattern};

    const INVOKE_LABEL: &str = "InvokePattern.Invoke";
    const LEGACY_LABEL: &str = "LegacyIAccessible.DoDefaultAction";

    pub(crate) fn execute_action_impl(
        handle: &NativeHandle,
        request: ActionRequest,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        if handle.is_null() {
            return null_handle_action(&request.action);
        }
        let element = uia_element(handle)?;
        let deadline = lease.deadline();
        ensure_budget(deadline)?;
        corroborate_verified_process(element)?;
        match &request.action {
            Action::Click => execute_click(element, request.policy, deadline),
            Action::SetFocus => focus_element(element, &request, deadline),
            Action::ScrollTo => execute_scroll_to(handle, lease),
            Action::TypeText(_) => Err(AdapterError::not_supported("key synthesis")),
            Action::PressKey(_) => Err(AdapterError::not_supported("key synthesis")),
            Action::DoubleClick | Action::TripleClick => {
                Err(AdapterError::not_supported("multi-click"))
            }
            Action::RightClick => Err(AdapterError::not_supported("physical context-menu click")),
            Action::KeyDown(_) | Action::KeyUp(_) | Action::Hover | Action::Drag(_) => {
                adapter_level_rejection(request.action.name())
            }
            Action::SetValue(value) => execute_set_value(element, value, request.policy, deadline),
            Action::Clear => execute_clear(element, request.policy, deadline),
            Action::Toggle => execute_toggle(element, request.policy, deadline),
            Action::Check => execute_check(element, request.policy, deadline),
            Action::Uncheck => execute_uncheck(element, request.policy, deadline),
            Action::Expand => execute_expand(element, request.policy, deadline),
            Action::Collapse => execute_collapse(element, request.policy, deadline),
            Action::Select(value) => execute_select(element, value, deadline),
            Action::Scroll(direction, amount) => {
                execute_scroll(element, direction, *amount, request.policy, deadline)
            }
        }
    }

    fn execute_select(
        element: &UIAElement,
        value: &str,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let steps = select_steps(element, value, deadline)?;
        ActionResult::from_execution(&Action::Select(value.to_string()), steps, None)
    }

    fn execute_scroll(
        element: &UIAElement,
        direction: &Direction,
        amount: u32,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let steps = scroll_steps(element, direction, amount, policy, deadline)?;
        ActionResult::from_execution(&Action::Scroll(direction.clone(), amount), steps, None)
    }

    fn execute_set_value(
        element: &UIAElement,
        value: &str,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let action = Action::SetValue(value.to_string());
        let steps = set_value_steps(element, value, policy, deadline)?;
        let post_state = post_state_for_steps(element, &action, &steps, deadline)?;
        ActionResult::from_execution(&action, steps, post_state)
    }

    fn execute_clear(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let steps = clear_steps(element, policy, deadline)?;
        let post_state = post_state_for_steps(element, &Action::Clear, &steps, deadline)?;
        ActionResult::from_execution(&Action::Clear, steps, post_state)
    }

    fn execute_toggle(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let steps = toggle_steps(element, policy, deadline)?;
        let post_state = post_state_for_steps(element, &Action::Toggle, &steps, deadline)?;
        ActionResult::from_execution(&Action::Toggle, steps, post_state)
    }

    fn execute_check(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let steps = check_steps(element, policy, deadline)?;
        let post_state = post_state_for_steps(element, &Action::Check, &steps, deadline)?;
        ActionResult::from_execution(&Action::Check, steps, post_state)
    }

    fn execute_uncheck(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let steps = uncheck_steps(element, policy, deadline)?;
        let post_state = post_state_for_steps(element, &Action::Uncheck, &steps, deadline)?;
        ActionResult::from_execution(&Action::Uncheck, steps, post_state)
    }

    fn execute_expand(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let steps = expand_steps(element, policy, deadline)?;
        let post_state = post_state_for_steps(element, &Action::Expand, &steps, deadline)?;
        ActionResult::from_execution(&Action::Expand, steps, post_state)
    }

    fn execute_collapse(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let steps = collapse_steps(element, policy, deadline)?;
        let post_state = post_state_for_steps(element, &Action::Collapse, &steps, deadline)?;
        ActionResult::from_execution(&Action::Collapse, steps, post_state)
    }

    fn null_handle_action(action: &Action) -> Result<ActionResult, AdapterError> {
        match action {
            Action::PressKey(_) => Err(AdapterError::not_supported("key synthesis")),
            _ => Err(invalid_native_handle(true)),
        }
    }

    fn invalid_native_handle(empty: bool) -> AdapterError {
        AdapterError::new(
            ErrorCode::InvalidArgs,
            "Native handle does not contain a Windows UI Automation element",
        )
        .with_details(serde_json::json!({
            "kind": "invalid_native_handle",
            "platform": "windows",
            "empty": empty
        }))
    }

    fn adapter_level_rejection(label: &str) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            format!("{label} requires adapter-level handling, not element action"),
        )
        .with_suggestion(
            "Use the top-level command (e.g. 'hover', 'drag', 'key-down') instead of targeting an element.",
        ))
    }

    fn execute_scroll_to(
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        let done = scroll_into_view_outcome(handle, lease)?;
        ActionResult::from_execution(
            &Action::ScrollTo,
            vec![build_step(done.label, done.outcome)],
            None,
        )
    }

    fn execute_click(
        element: &UIAElement,
        policy: InteractionPolicy,
        deadline: Deadline,
    ) -> Result<ActionResult, AdapterError> {
        let invoke_available = click_invoke_available(element);
        let legacy_available = click_legacy_available(element);
        let steps = click_chain_judged_for(
            deadline,
            policy,
            invoke_available,
            legacy_available,
            || invoke_pattern(element),
            || legacy_default_action(element),
        )?;
        ActionResult::from_execution(&Action::Click, steps, None)
    }

    pub(crate) fn click_invoke_available(element: &UIAElement) -> bool {
        read_one(element, TreeProperty::InvokeAvailable).flag() == Some(true)
    }

    pub(crate) fn click_legacy_available(element: &UIAElement) -> bool {
        matches!(
            read_one(element, TreeProperty::LegacyDefaultAction).text(),
            LocatorField::Known(text) if !text.trim().is_empty()
        )
    }

    /// Click chain with injected rungs — unit-test seam and live path.
    pub(crate) fn click_chain_judged_for(
        deadline: Deadline,
        policy: InteractionPolicy,
        invoke_available: bool,
        legacy_available: bool,
        mut invoke: impl FnMut() -> Result<DeliveryOutcome, AdapterError>,
        mut legacy: impl FnMut() -> Result<DeliveryOutcome, AdapterError>,
    ) -> Result<Vec<ActionStep>, AdapterError> {
        let mut invoke_run = || {
            if !invoke_available {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            invoke()
        };
        let mut legacy_run = || {
            if !legacy_available {
                return Ok(DeliveryOutcome::NotDelivered);
            }
            legacy()
        };
        execute_chain(
            deadline,
            &CLICK_CHAIN,
            policy,
            &mut [
                ChainRung {
                    label: INVOKE_LABEL,
                    requires_headed: false,
                    run: &mut invoke_run,
                },
                ChainRung {
                    label: LEGACY_LABEL,
                    requires_headed: false,
                    run: &mut legacy_run,
                },
            ],
        )
    }

    fn invoke_pattern(element: &UIAElement) -> Result<DeliveryOutcome, AdapterError> {
        let delivered = match element.0.get_pattern::<UIInvokePattern>() {
            Ok(pattern) => match pattern.invoke() {
                Ok(()) => classify_success()?,
                Err(error) => classify_write("Invoke", INVOKE_LABEL, &error)?,
            },
            Err(error) => classify_write("get_pattern", INVOKE_LABEL, &error)?,
        };
        Ok(DeliveryOutcome::from_delivery(delivered, false))
    }

    fn legacy_default_action(element: &UIAElement) -> Result<DeliveryOutcome, AdapterError> {
        let delivered = match element.0.get_pattern::<UILegacyIAccessiblePattern>() {
            Ok(pattern) => match pattern.do_default_action() {
                Ok(()) => classify_success()?,
                Err(error) => classify_write("DoDefaultAction", LEGACY_LABEL, &error)?,
            },
            Err(error) => classify_write("get_pattern", LEGACY_LABEL, &error)?,
        };
        Ok(DeliveryOutcome::from_delivery(delivered, false))
    }

    fn classify_write(
        operation: &str,
        api: &str,
        error: &uiautomation::Error,
    ) -> Result<bool, AdapterError> {
        match failure_of(error) {
            UiaFailure::Sentinel(ERR_NONE) => Ok(false),
            other if other.is_exhaustion() => Ok(false),
            failure => classify_mutation(operation, api, &failure),
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod imp {
    use super::{ActionRequest, ActionResult, AdapterError, InteractionLease, NativeHandle};

    pub(crate) fn execute_action_impl(
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Err(AdapterError::not_supported("execute_action"))
    }
}

pub(crate) use imp::execute_action_impl;

#[cfg(all(test, target_os = "windows"))]
pub(crate) use imp::click_chain_judged_for;

#[cfg(all(test, target_os = "windows"))]
#[path = "dispatch_tests.rs"]
mod tests;
