use super::ActionabilityStatus;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ActionabilityCheck {
    pub name: &'static str,
    pub status: ActionabilityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
