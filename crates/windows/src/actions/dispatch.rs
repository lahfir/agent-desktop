//! `execute_action` match over every `Action` variant.
//!
//! Click routes through the Invoke + Legacy chain. SetValue / Clear route
//! through `value_write` with post-state attachment. SetFocus routes through
//! `focus`. ScrollTo reuses the shipped ScrollIntoView spine. Capabilities
//! that need key synthesis or physical multi-click fail
//! `PLATFORM_NOT_SUPPORTED` naming the missing machinery. Toggle / disclosure
//! / select / scroll modules replace their placeholder arms when those
//! modules land — the placeholders are deliberate `ACTION_FAILED` outcomes,
//! never the trait-default `execute_action` message.

use agent_desktop_core::{
    Action, ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
    InteractionLease, InteractionPolicy, NativeHandle, StepMechanism, action_request::ActionRequest,
};

#[cfg(target_os = "windows")]
mod imp {
    use super::{
        Action, ActionResult, ActionStep, AdapterError, Deadline, DeliverySemantics, ErrorCode,
        InteractionLease, InteractionPolicy, NativeHandle, StepMechanism, ActionRequest,
    };
    use crate::actions::chain::{
        CLICK_CHAIN, ChainRung, DeliveryOutcome, execute_chain,
    };
    use crate::actions::focus::focus_element;
    use crate::actions::mutation::{classify_mutation, classify_success};
    use crate::actions::post_state::post_state_for_steps;
    use crate::actions::scroll_into_view::scroll_into_view_impl;
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
            Action::RightClick => {
                Err(AdapterError::not_supported("physical context-menu click"))
            }
            Action::KeyDown(_) | Action::KeyUp(_) | Action::Hover | Action::Drag(_) => {
                adapter_level_rejection(request.action.name())
            }
            Action::SetValue(value) => {
                execute_set_value(element, value, request.policy, deadline)
            }
            Action::Clear => execute_clear(element, request.policy, deadline),
            Action::Toggle | Action::Check | Action::Uncheck => Err(unwired("toggle")),
            Action::Expand | Action::Collapse => Err(unwired("disclosure")),
            Action::Select(_) => Err(unwired("select")),
            Action::Scroll(_, _) => Err(unwired("scroll")),
        }
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

    fn unwired(module: &str) -> AdapterError {
        AdapterError::new(
            ErrorCode::ActionFailed,
            format!("{module} action awaits a dedicated semantic module"),
        )
        .with_disposition(DeliverySemantics::not_delivered())
        .with_suggestion(
            "Use a capability that is already wired, or target an element whose advertised action is Click, SetValue, Clear, SetFocus, or ScrollTo.",
        )
    }

    fn execute_scroll_to(
        handle: &NativeHandle,
        lease: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        scroll_into_view_impl(handle, lease)?;
        ActionResult::from_execution(
            &Action::ScrollTo,
            vec![
                ActionStep::succeeded("ScrollItemPattern.ScrollIntoView")
                    .with_mechanism(StepMechanism::SemanticApi)
                    .with_verified(true),
            ],
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
    use super::{ActionResult, AdapterError, InteractionLease, NativeHandle, ActionRequest};

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
