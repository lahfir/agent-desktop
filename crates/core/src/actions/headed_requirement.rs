use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeadedRequirement {
    None,
    FocusedApplication,
    FocusedApplicationAndCursor,
    FocusedWindow,
    FocusedWindowAndCursor,
}

impl HeadedRequirement {
    pub fn requires_focus(self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn requires_cursor(self) -> bool {
        matches!(
            self,
            Self::FocusedApplicationAndCursor | Self::FocusedWindowAndCursor
        )
    }
}
