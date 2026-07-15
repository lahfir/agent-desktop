use super::{PointerDelivery, check::ActionabilityCheck, status::ActionabilityStatus};
use crate::{ErrorCode, Point};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub(crate) struct ActionabilityReport {
    pub(crate) actionable: bool,
    pub(crate) checks: Vec<ActionabilityCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) verified_point: Option<Point>,
    #[serde(skip)]
    pub(crate) pointer_delivery: PointerDelivery,
}

impl ActionabilityReport {
    pub(crate) fn from_checks(
        checks: Vec<ActionabilityCheck>,
        verified_point: Option<Point>,
        pointer_delivery: PointerDelivery,
    ) -> Self {
        let actionable = checks.iter().all(|check| !is_blocking(check));
        Self {
            actionable,
            checks,
            verified_point,
            pointer_delivery,
        }
    }

    pub(crate) fn terminal_code(&self) -> Option<ErrorCode> {
        self.checks
            .iter()
            .find(|check| is_blocking(check))
            .and_then(|check| check.terminal_code.clone())
    }

    pub(crate) fn failure_reasons(&self) -> String {
        self.checks
            .iter()
            .filter(|check| is_blocking(check))
            .map(|check| {
                let reason = check.reason.as_deref().unwrap_or("failed");
                format!("{} ({reason})", check.check)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn is_blocking(check: &ActionabilityCheck) -> bool {
    matches!(check.status, ActionabilityStatus::Fail)
        || matches!(check.status, ActionabilityStatus::Unknown)
            && (check.check != "receives_events"
                || check
                    .hit_test
                    .as_ref()
                    .is_some_and(|evidence| evidence.occluded > 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(check: &'static str, terminal_code: Option<ErrorCode>) -> ActionabilityCheck {
        ActionabilityCheck {
            check,
            status: ActionabilityStatus::Fail,
            reason: None,
            occluder: None,
            terminal_code,
            hit_test: None,
            stability: None,
        }
    }

    #[test]
    fn retryable_failure_is_not_overridden_by_a_later_terminal_check() {
        let report = ActionabilityReport::from_checks(
            vec![
                failed("enabled", None),
                failed("supported_action", Some(ErrorCode::PolicyDenied)),
            ],
            None,
            PointerDelivery::NotApplicable,
        );

        assert_eq!(report.terminal_code(), None);
    }
}
