use serde::{Deserialize, Serialize};

use crate::Rect;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodePresentation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub states: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub available_actions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect>,
}
