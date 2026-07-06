use super::{ActionabilityCheck, ActionabilityStatus};
use crate::error::ErrorCode;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ActionabilityReport {
    pub actionable: bool,
    pub checks: Vec<ActionabilityCheck>,
}

impl ActionabilityReport {
    /// The error code for a non-actionable report: the first failing check that
    /// declared its failure terminal (permanent — waiting cannot heal it), or
    /// `None` when every failure is transient and the action should be retried
    /// within the auto-wait budget.
    pub(crate) fn terminal_code(&self) -> Option<ErrorCode> {
        self.checks
            .iter()
            .filter(|check| matches!(check.status, ActionabilityStatus::Fail))
            .find_map(|check| check.terminal_code.clone())
    }
}
