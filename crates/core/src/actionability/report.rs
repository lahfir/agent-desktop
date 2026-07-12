use super::{
    check::ActionabilityCheck, requirements::PointerDelivery, status::ActionabilityStatus,
};
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
        let actionable = checks
            .iter()
            .all(|check| matches!(check.status, ActionabilityStatus::Pass));
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
            .filter(|check| !matches!(check.status, ActionabilityStatus::Pass))
            .find_map(|check| check.terminal_code.clone())
    }

    pub(crate) fn failure_reasons(&self) -> String {
        self.checks
            .iter()
            .filter(|check| !matches!(check.status, ActionabilityStatus::Pass))
            .map(|check| {
                let reason = check.reason.as_deref().unwrap_or("failed");
                format!("{} ({reason})", check.check)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}
