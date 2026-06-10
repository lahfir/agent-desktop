use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionPolicy {
    pub allow_focus_steal: bool,
    pub allow_cursor_move: bool,
}

impl InteractionPolicy {
    pub fn headless() -> Self {
        Self {
            allow_focus_steal: false,
            allow_cursor_move: false,
        }
    }

    pub fn focus_fallback() -> Self {
        Self {
            allow_focus_steal: true,
            allow_cursor_move: false,
        }
    }

    pub fn physical() -> Self {
        Self {
            allow_focus_steal: true,
            allow_cursor_move: true,
        }
    }
}

impl Default for InteractionPolicy {
    fn default() -> Self {
        Self::headless()
    }
}
