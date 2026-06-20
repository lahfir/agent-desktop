use super::ActionabilityCheck;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ActionabilityReport {
    pub actionable: bool,
    pub checks: Vec<ActionabilityCheck>,
}
