use agent_desktop_core::{AdapterError, ErrorCode};

pub(crate) fn fallback_is_safe(error: &AdapterError) -> bool {
    error.code == ErrorCode::ActionFailed
        && error.disposition == agent_desktop_core::DeliverySemantics::not_delivered()
}

#[cfg(test)]
mod tests {
    use agent_desktop_core::{AdapterError, ErrorCode};

    use super::fallback_is_safe;

    #[test]
    fn rejects_uncertain_or_non_action_failures() {
        let uncertain = AdapterError::new(ErrorCode::ActionFailed, "uncertain")
            .with_disposition(agent_desktop_core::DeliverySemantics::uncertain());
        assert!(!fallback_is_safe(&uncertain));
        assert!(!fallback_is_safe(&AdapterError::permission_denied()));
        assert!(!fallback_is_safe(&AdapterError::new(
            ErrorCode::AppUnresponsive,
            "unresponsive",
        )));
    }

    #[test]
    fn accepts_definite_non_delivery() {
        let not_delivered = AdapterError::new(ErrorCode::ActionFailed, "not delivered")
            .with_disposition(agent_desktop_core::DeliverySemantics::not_delivered());
        assert!(fallback_is_safe(&not_delivered));
    }
}
