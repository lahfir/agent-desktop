use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActionabilityStatus {
    Pass,
    Fail,
    Unknown,
}
