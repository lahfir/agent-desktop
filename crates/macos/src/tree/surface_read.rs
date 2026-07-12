use agent_desktop_core::{AdapterError, ErrorCode};
use std::time::Instant;

pub(crate) fn elements(
    element: &super::AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Vec<super::AXElement>, AdapterError> {
    let read = super::query::child_read::read_attribute_children(
        element,
        attribute,
        agent_desktop_core::ObservationBudget::default().max_children_per_node,
        deadline,
    );
    ensure_before_deadline(deadline)?;
    if read.status.api_disabled {
        return Err(map_error(attribute, accessibility_sys::kAXErrorAPIDisabled));
    }
    if read.status.invalid_element {
        return Err(map_error(
            attribute,
            accessibility_sys::kAXErrorInvalidUIElement,
        ));
    }
    if !read.complete || read.truncated() {
        return Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            format!("Accessibility surface read was incomplete for {attribute}"),
        )
        .with_details(serde_json::json!({
            "kind": "surface_array_incomplete",
            "attribute": attribute,
            "complete": false,
            "total_count": read.total_count,
            "loaded_count": read.elements.len(),
            "count_changed": read.status.count_changed,
        })));
    }
    Ok(read.elements)
}

pub(crate) fn element(
    source: &super::AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<super::AXElement>, AdapterError> {
    prepare(source, deadline)?;
    finish(
        super::attributes::copy_element_attr_result(source, attribute, deadline),
        attribute,
        deadline,
    )
}

pub(crate) fn string(
    element: &super::AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<String>, AdapterError> {
    prepare(element, deadline)?;
    finish(
        super::attributes::copy_string_attr_result(element, attribute, deadline),
        attribute,
        deadline,
    )
}

pub(crate) fn boolean(
    element: &super::AXElement,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<bool>, AdapterError> {
    prepare(element, deadline)?;
    finish(
        super::attributes::copy_bool_attr_result(element, attribute, deadline),
        attribute,
        deadline,
    )
}

pub(crate) fn ensure_before_deadline(deadline: Instant) -> Result<(), AdapterError> {
    if Instant::now() >= deadline {
        return Err(deadline_error());
    }
    Ok(())
}

fn prepare(element: &super::AXElement, deadline: Instant) -> Result<(), AdapterError> {
    if deadline.saturating_duration_since(Instant::now()).is_zero() {
        return Err(deadline_error());
    }
    super::attributes::set_messaging_timeout(element, deadline)?;
    Ok(())
}

fn finish<T>(
    result: Result<Option<T>, i32>,
    attribute: &str,
    deadline: Instant,
) -> Result<Option<T>, AdapterError> {
    ensure_before_deadline(deadline)?;
    result.map_err(|error| map_error(attribute, error))
}

fn map_error(attribute: &str, error: i32) -> AdapterError {
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorInvalidUIElement,
    };

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
        format!("Accessibility surface read failed for {attribute}"),
    )
    .with_details(serde_json::json!({
        "attribute": attribute,
        "ax_error": error,
        "kind": "surface_read",
    }))
    .with_suggestion("Retry after the application finishes updating its accessibility surfaces")
}

fn deadline_error() -> AdapterError {
    AdapterError::timeout("Accessibility surface resolution exceeded its absolute deadline")
}

#[cfg(test)]
mod tests {
    use super::*;
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorCannotComplete, kAXErrorInvalidUIElement,
    };

    #[test]
    fn classifier_preserves_permission_timeout_and_stale_states() {
        assert_eq!(
            map_error("AXRole", kAXErrorAPIDisabled).code,
            ErrorCode::PermDenied
        );
        assert_eq!(
            map_error("AXRole", kAXErrorCannotComplete).code,
            ErrorCode::Timeout
        );
        assert_eq!(
            map_error("AXRole", kAXErrorInvalidUIElement).code,
            ErrorCode::ElementNotFound
        );
    }

    #[test]
    fn expired_deadline_fails_without_native_reads() {
        let error = ensure_before_deadline(Instant::now()).expect_err("expired deadline");

        assert_eq!(error.code, ErrorCode::Timeout);
    }
}
