use agent_desktop_core::{AdapterError, Deadline, ErrorCode};
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
use accessibility_sys::{
    AXUIElementCopyActionNames, AXUIElementCopyAttributeValue, AXUIElementCopyAttributeValues,
    AXUIElementCopyElementAtPosition, AXUIElementCopyMultipleAttributeValues,
    AXUIElementGetAttributeValueCount, AXUIElementGetPid, AXUIElementIsAttributeSettable,
    AXUIElementPerformAction, AXUIElementSetAttributeValue, AXUIElementSetMessagingTimeout,
    kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorFailure, kAXErrorInvalidUIElement,
    kAXErrorSuccess,
};
#[cfg(target_os = "macos")]
use core_foundation_sys::{
    array::CFArrayRef,
    base::{CFIndex, CFTypeRef},
    string::CFStringRef,
};

const MAX_IPC_SLICE: Duration = Duration::from_millis(250);

pub(crate) trait AxDeadline: Copy {
    fn absolute(self) -> Result<Instant, AdapterError>;
}

impl AxDeadline for Instant {
    fn absolute(self) -> Result<Instant, AdapterError> {
        Ok(self)
    }
}

impl AxDeadline for Deadline {
    fn absolute(self) -> Result<Instant, AdapterError> {
        let remaining = self.remaining();
        if remaining.is_zero() {
            return Err(self.timeout_error());
        }
        Instant::now()
            .checked_add(remaining)
            .ok_or_else(|| AdapterError::timeout("Accessibility deadline overflowed"))
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn prepare(
    element: &super::AXElement,
    deadline: impl AxDeadline,
) -> Result<Duration, AdapterError> {
    if element.0.is_null() {
        return Err(AdapterError::new(
            ErrorCode::ElementNotFound,
            "Cannot address a null accessibility element",
        ));
    }
    let remaining = deadline
        .absolute()?
        .saturating_duration_since(Instant::now())
        .min(MAX_IPC_SLICE);
    if remaining.is_zero() {
        return Err(AdapterError::timeout(
            "Accessibility deadline exhausted before IPC",
        ));
    }
    let error = unsafe { AXUIElementSetMessagingTimeout(element.0, remaining.as_secs_f32()) };
    if error == kAXErrorSuccess {
        Ok(remaining)
    } else {
        Err(timeout_install_error(error))
    }
}

#[cfg(target_os = "macos")]
pub(crate) fn copy_attribute_value(
    element: &super::AXElement,
    attribute: CFStringRef,
    deadline: impl AxDeadline,
) -> (i32, CFTypeRef) {
    let mut value: CFTypeRef = std::ptr::null();
    let error = match prepare(element, deadline) {
        Ok(_) => unsafe { AXUIElementCopyAttributeValue(element.0, attribute, &mut value) },
        Err(error) => adapter_error_to_ax(&error),
    };
    (error, value)
}

#[cfg(target_os = "macos")]
pub(crate) fn copy_attribute_values(
    element: &super::AXElement,
    attribute: CFStringRef,
    index: CFIndex,
    max_values: CFIndex,
    deadline: impl AxDeadline,
) -> (i32, CFArrayRef) {
    let mut values = std::ptr::null();
    let error = match prepare(element, deadline) {
        Ok(_) => unsafe {
            AXUIElementCopyAttributeValues(element.0, attribute, index, max_values, &mut values)
        },
        Err(error) => adapter_error_to_ax(&error),
    };
    (error, values)
}

#[cfg(target_os = "macos")]
pub(crate) fn copy_multiple_attribute_values(
    element: &super::AXElement,
    attributes: CFArrayRef,
    deadline: impl AxDeadline,
) -> (i32, CFTypeRef) {
    let mut values: CFTypeRef = std::ptr::null();
    let error = match prepare(element, deadline) {
        Ok(_) => unsafe {
            AXUIElementCopyMultipleAttributeValues(
                element.0,
                attributes,
                0,
                &mut values as *mut _ as *mut _,
            )
        },
        Err(error) => adapter_error_to_ax(&error),
    };
    (error, values)
}

#[cfg(target_os = "macos")]
pub(crate) fn attribute_value_count(
    element: &super::AXElement,
    attribute: CFStringRef,
    deadline: impl AxDeadline,
) -> Result<usize, i32> {
    let mut count: CFIndex = 0;
    let error = match prepare(element, deadline) {
        Ok(_) => unsafe { AXUIElementGetAttributeValueCount(element.0, attribute, &mut count) },
        Err(error) => adapter_error_to_ax(&error),
    };
    if error != kAXErrorSuccess {
        return Err(error);
    }
    usize::try_from(count).map_err(|_| i32::MIN)
}

#[cfg(target_os = "macos")]
pub(crate) fn copy_action_names(
    element: &super::AXElement,
    deadline: impl AxDeadline,
) -> (i32, CFArrayRef) {
    let mut actions = std::ptr::null();
    let error = match prepare(element, deadline) {
        Ok(_) => unsafe { AXUIElementCopyActionNames(element.0, &mut actions) },
        Err(error) => adapter_error_to_ax(&error),
    };
    (error, actions)
}

#[cfg(target_os = "macos")]
pub(crate) fn is_attribute_settable(
    element: &super::AXElement,
    attribute: CFStringRef,
    deadline: impl AxDeadline,
) -> (i32, bool) {
    let mut settable = 0_u8;
    let error = match prepare(element, deadline) {
        Ok(_) => unsafe { AXUIElementIsAttributeSettable(element.0, attribute, &mut settable) },
        Err(error) => adapter_error_to_ax(&error),
    };
    (error, settable != 0)
}

#[cfg(target_os = "macos")]
pub(crate) fn perform_action(
    element: &super::AXElement,
    action: CFStringRef,
    deadline: impl AxDeadline,
) -> Result<i32, AdapterError> {
    prepare(element, deadline).map_err(pre_mutation_error)?;
    Ok(unsafe { AXUIElementPerformAction(element.0, action) })
}

#[cfg(target_os = "macos")]
pub(crate) fn set_attribute_value(
    element: &super::AXElement,
    attribute: CFStringRef,
    value: CFTypeRef,
    deadline: impl AxDeadline,
) -> Result<i32, AdapterError> {
    prepare(element, deadline).map_err(pre_mutation_error)?;
    Ok(unsafe { AXUIElementSetAttributeValue(element.0, attribute, value) })
}

#[cfg(target_os = "macos")]
pub(crate) fn element_at_position(
    system: &super::AXElement,
    point: (f32, f32),
    deadline: impl AxDeadline,
) -> (i32, accessibility_sys::AXUIElementRef) {
    let mut element = std::ptr::null_mut();
    let error = match prepare(system, deadline) {
        Ok(_) => unsafe {
            AXUIElementCopyElementAtPosition(system.0, point.0, point.1, &mut element)
        },
        Err(error) => adapter_error_to_ax(&error),
    };
    (error, element)
}

#[cfg(target_os = "macos")]
pub(crate) fn pid(element: &super::AXElement, deadline: impl AxDeadline) -> Result<i32, i32> {
    let mut pid = 0_i32;
    let error = match prepare(element, deadline) {
        Ok(_) => unsafe { AXUIElementGetPid(element.0, &mut pid) },
        Err(error) => adapter_error_to_ax(&error),
    };
    if error == kAXErrorSuccess && pid > 0 {
        Ok(pid)
    } else {
        Err(error)
    }
}

#[cfg(target_os = "macos")]
fn timeout_install_error(error: i32) -> AdapterError {
    let code = if error == kAXErrorAPIDisabled {
        ErrorCode::PermDenied
    } else if error == kAXErrorCannotComplete {
        ErrorCode::Timeout
    } else if error == kAXErrorInvalidUIElement {
        ErrorCode::ElementNotFound
    } else {
        ErrorCode::ActionFailed
    };
    AdapterError::new(
        code,
        "Could not install the accessibility messaging timeout",
    )
    .with_details(serde_json::json!({
        "ax_error": error,
        "kind": "messaging_timeout_install",
    }))
    .with_suggestion("Refresh the target and retry before issuing another accessibility call")
}

#[cfg(target_os = "macos")]
fn adapter_error_to_ax(error: &AdapterError) -> i32 {
    match error.code {
        ErrorCode::PermDenied => kAXErrorAPIDisabled,
        ErrorCode::ElementNotFound | ErrorCode::StaleRef => kAXErrorInvalidUIElement,
        ErrorCode::Timeout | ErrorCode::AppUnresponsive => kAXErrorCannotComplete,
        _ => kAXErrorFailure,
    }
}

fn pre_mutation_error(error: AdapterError) -> AdapterError {
    error.with_disposition(agent_desktop_core::DeliverySemantics::not_delivered())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_preflight_failure_is_explicitly_not_delivered() {
        let error = pre_mutation_error(AdapterError::timeout("preflight"));

        assert_eq!(
            error.disposition,
            agent_desktop_core::DeliverySemantics::not_delivered()
        );
    }
}
