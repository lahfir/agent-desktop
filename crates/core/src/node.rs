use serde::{Deserialize, Serialize};

#[cfg(test)]
use crate::{AppInfo, Rect};
use crate::{NodeIdentity, NodePresentation};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccessibilityNode {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,

    pub role: String,

    #[serde(flatten)]
    pub identity: NodeIdentity,

    #[serde(flatten)]
    pub presentation: NodePresentation,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub children_count: Option<u32>,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub children: Vec<AccessibilityNode>,
}

#[cfg(test)]
#[path = "node_tests.rs"]
mod tests;
