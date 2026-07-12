use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowState {
    pub is_focused: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minimized: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}
