use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocatorMatchData {
    pub ref_id: Option<String>,
    pub role: String,
    pub name: String,
    pub value: Option<String>,
    pub states: Vec<String>,
    pub interactive: bool,
    pub path: Vec<String>,
}
