use accessibility_sys::{
    kAXErrorAPIDisabled, kAXErrorActionUnsupported, kAXErrorAttributeUnsupported,
    kAXErrorCannotComplete, kAXErrorFailure, kAXErrorIllegalArgument, kAXErrorInvalidUIElement,
    kAXErrorNoValue, kAXErrorNotImplemented, kAXErrorSuccess,
};
use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

pub(crate) const PERFORM_API: &str = "AXUIElementPerformAction";

pub(crate) fn classify_result(
    _element: &crate::tree::AXElement,
    operation: &str,
    api: &str,
    error: i32,
) -> Result<bool, AdapterError> {
    classify(operation, api, error)
}

fn classify(operation: &str, api: &str, error: i32) -> Result<bool, AdapterError> {
    signal_or_error(operation, api, error).map(|signal| signal == PerformSignal::ReportedDelivered)
}

fn signal_or_error(operation: &str, api: &str, error: i32) -> Result<PerformSignal, AdapterError> {
    if error == kAXErrorSuccess {
        return Ok(PerformSignal::ReportedDelivered);
    }
    if error == kAXErrorActionUnsupported
        || error == kAXErrorNotImplemented
        || error == kAXErrorFailure
    {
        return Ok(PerformSignal::ReportedUnsupported);
    }
    if error == kAXErrorAttributeUnsupported || error == kAXErrorNoValue {
        return Ok(PerformSignal::Uninformative);
    }
    if error == kAXErrorAPIDisabled {
        return Err(AdapterError::permission_denied()
            .with_platform_detail(format!(
                "{api}({operation}) failed with kAXErrorAPIDisabled"
            ))
            .with_disposition(DeliverySemantics::not_delivered()));
    }
    if error == kAXErrorInvalidUIElement {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            format!("{operation} targeted an invalid accessibility element"),
        )
        .with_details(serde_json::json!({
            "ax_error": "kAXErrorInvalidUIElement",
            "operation": operation,
        }))
        .with_disposition(DeliverySemantics::not_delivered())
        .with_suggestion("Refresh the snapshot and retry with the new element reference."));
    }
    if error == kAXErrorIllegalArgument {
        return Err(AdapterError::new(
            ErrorCode::InvalidArgs,
            format!("{operation} rejected an invalid accessibility argument"),
        )
        .with_platform_detail(format!("{api}({operation}) returned {error}"))
        .with_details(serde_json::json!({
            "ax_error": error,
            "operation": operation,
        }))
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    let (code, label) = if error == kAXErrorCannotComplete {
        (ErrorCode::AppUnresponsive, "kAXErrorCannotComplete")
    } else {
        (ErrorCode::ActionFailed, "unclassified AXError")
    };
    Err(crate::actions::DeliveryTracker::uncertain(
        AdapterError::new(
            code,
            format!("{operation} returned {label}; mutation outcome is uncertain"),
        )
        .with_details(serde_json::json!({
            "ax_error": error,
            "operation": operation,
        }))
        .with_platform_detail(format!("{api}({operation}) returned {error}"))
        .with_suggestion(
            "Inspect the target state with a fresh snapshot before deciding whether to retry.",
        ),
    ))
}

/// How much a perform return code proves. Responders break the contract in both
/// directions — a code that reports failure after acting, and success after
/// doing nothing — so delivery is settled by observation, not by the code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PerformSignal {
    ReportedDelivered,
    ReportedUnsupported,
    Uninformative,
}

pub(crate) fn classify_perform(operation: &str, error: i32) -> Result<PerformSignal, AdapterError> {
    signal_or_error(operation, PERFORM_API, error)
}

#[cfg(test)]
mod tests {
    use accessibility_sys::{
        kAXErrorAPIDisabled, kAXErrorActionUnsupported, kAXErrorAttributeUnsupported,
        kAXErrorCannotComplete, kAXErrorInvalidUIElement, kAXErrorSuccess,
    };
    use agent_desktop_core::{DeliveryDisposition, ErrorCode, RetryDisposition};

    use super::classify;

    #[test]
    fn success_is_delivered() {
        let result = classify("AXPress", "perform", kAXErrorSuccess);
        assert!(result.unwrap());
    }

    #[test]
    fn unsupported_action_remains_safe_non_delivery() {
        let result = classify("AXPress", "perform", kAXErrorActionUnsupported);
        assert!(!result.unwrap());
    }

    #[test]
    fn perform_codes_are_graded_by_how_much_they_prove() {
        use super::{PerformSignal, classify_perform};

        assert_eq!(
            classify_perform("AXPress", kAXErrorSuccess).unwrap(),
            PerformSignal::ReportedDelivered
        );
        assert_eq!(
            classify_perform("AXPress", kAXErrorActionUnsupported).unwrap(),
            PerformSignal::ReportedUnsupported
        );
        assert_eq!(
            classify_perform("AXOpen", kAXErrorAttributeUnsupported).unwrap(),
            PerformSignal::Uninformative
        );
    }

    /// `tree::action_list` reads `kAXErrorFailure` as "this element implements
    /// no such action". A perform must not read the same code as a failure, or
    /// an element the reader called action-less becomes an error when acted on.
    #[test]
    fn perform_and_capability_reads_agree_on_the_appkit_absence_code() {
        assert_eq!(
            super::classify_perform("AXScrollToVisible", accessibility_sys::kAXErrorFailure)
                .unwrap(),
            super::PerformSignal::ReportedUnsupported
        );
    }

    #[test]
    fn perform_keeps_reporting_transport_failures_as_errors() {
        let error = super::classify_perform("AXOpen", kAXErrorInvalidUIElement)
            .expect_err("a stale element must still fail closed");
        assert_eq!(error.code, ErrorCode::StaleRef);
    }

    #[test]
    fn invalid_element_is_stale_and_safe_to_retry_with_a_fresh_ref() {
        let error = classify("AXPress", "perform", kAXErrorInvalidUIElement)
            .expect_err("stale element must fail closed");

        assert_eq!(error.code, ErrorCode::StaleRef);
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::NotDelivered
        );
        assert_eq!(error.disposition.retry(), RetryDisposition::Safe);
    }

    #[test]
    fn api_disabled_remains_permission_denied_without_probe() {
        let error = classify("AXPress", "perform", kAXErrorAPIDisabled).unwrap_err();
        assert_eq!(error.code, ErrorCode::PermDenied);
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::NotDelivered
        );
    }

    #[test]
    fn cannot_complete_reports_uncertainty_without_an_extra_read() {
        let error = classify("AXValue", "set", kAXErrorCannotComplete).unwrap_err();
        assert_eq!(error.code, ErrorCode::AppUnresponsive);
        assert_eq!(
            error.disposition.delivery(),
            DeliveryDisposition::DeliveryUncertain
        );
        assert_eq!(error.disposition.retry(), RetryDisposition::Unsafe);
        assert!(
            error
                .suggestion
                .as_deref()
                .unwrap()
                .contains("Inspect the target state")
        );
    }
}
