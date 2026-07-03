use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub value_promoted: bool,
    pub child_label: bool,
}

impl NameEvidence {
    pub fn resolved_name(&self) -> Option<String> {
        self.title.clone().or_else(|| self.description.clone())
    }
}
