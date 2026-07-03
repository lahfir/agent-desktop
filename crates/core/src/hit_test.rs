use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HitTestResult {
    pub receives_events: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topmost_role: Option<String>,
}

impl HitTestResult {
    pub fn receives_events(topmost_role: Option<String>) -> Self {
        Self {
            receives_events: true,
            topmost_role,
        }
    }

    pub fn blocked(topmost_role: Option<String>) -> Self {
        Self {
            receives_events: false,
            topmost_role,
        }
    }
}

#[cfg(test)]
#[path = "hit_test_tests.rs"]
mod tests;
