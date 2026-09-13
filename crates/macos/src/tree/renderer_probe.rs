use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

pub(crate) fn activation_supported(
    pid: i32,
    process_instance: &str,
    deadline: std::time::Instant,
) -> Result<bool, AdapterError> {
    if !crate::system::process_identity::matches_instance(pid, process_instance)? {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "Renderer process instance changed before activation probing",
        )
        .with_suggestion("Run 'snapshot' to refresh, then retry with the updated ref.")
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    let application = super::element_for_pid(pid);
    super::locator_deadline::prepare(&application, deadline)?;
    let read = super::capabilities::is_attr_settable_with_status(
        &application,
        "AXManualAccessibility",
        deadline,
    );
    super::locator_deadline::remaining(deadline)?;
    match (read.value, read.error) {
        (Some(value), None) => Ok(value),
        (None, Some(error)) if is_unsupported(error) => Ok(false),
        (None, Some(error)) if error == accessibility_sys::kAXErrorAPIDisabled => {
            Err(AdapterError::new(
                ErrorCode::PermDenied,
                "Accessibility API is disabled while probing renderer support",
            ))
        }
        (None, Some(error)) => Err(inconclusive_probe(error)),
        _ => Err(inconclusive_probe(accessibility_sys::kAXErrorFailure)),
    }
}

fn inconclusive_probe(error: i32) -> AdapterError {
    let code = match error {
        accessibility_sys::kAXErrorCannotComplete => ErrorCode::Timeout,
        accessibility_sys::kAXErrorInvalidUIElement => ErrorCode::StaleRef,
        _ => ErrorCode::AppUnresponsive,
    };
    AdapterError::new(
        code,
        "Renderer accessibility capability probe was inconclusive",
    )
    .with_details(serde_json::json!({
        "kind": "renderer_capability_probe",
        "ax_error": error,
        "complete": false,
        "retryable": true,
    }))
    .with_suggestion("Retry after the renderer accessibility tree finishes updating")
}

fn is_unsupported(error: i32) -> bool {
    matches!(
        error,
        accessibility_sys::kAXErrorAttributeUnsupported
            | accessibility_sys::kAXErrorNoValue
            | accessibility_sys::kAXErrorNotImplemented
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsupported_probe_results_do_not_request_activation() {
        for error in [
            accessibility_sys::kAXErrorAttributeUnsupported,
            accessibility_sys::kAXErrorNoValue,
            accessibility_sys::kAXErrorNotImplemented,
        ] {
            assert!(is_unsupported(error));
        }
        assert!(!is_unsupported(accessibility_sys::kAXErrorCannotComplete));
    }

    #[test]
    fn transient_probe_failures_are_never_collapsed_to_unsupported() {
        let error = inconclusive_probe(accessibility_sys::kAXErrorCannotComplete);

        assert_eq!(error.code, ErrorCode::Timeout);
        assert_eq!(error.details.unwrap()["complete"], false);
    }
}
