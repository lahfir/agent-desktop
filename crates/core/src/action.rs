use serde::{Deserialize, Serialize};

use crate::{Direction, DragParams, KeyCombo};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Click,
    DoubleClick,
    RightClick,
    TripleClick,
    SetValue(String),
    SetFocus,
    Expand,
    Collapse,
    Select(String),
    Toggle,
    Check,
    Uncheck,
    Scroll(Direction, u32),
    ScrollTo,
    PressKey(KeyCombo),
    KeyDown(KeyCombo),
    KeyUp(KeyCombo),
    TypeText(String),
    Clear,
    Hover,
    Drag(DragParams),
}

impl Action {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Click => "click",
            Self::DoubleClick => "double-click",
            Self::RightClick => "right-click",
            Self::TripleClick => "triple-click",
            Self::SetValue(_) => "set-value",
            Self::SetFocus => "focus",
            Self::Expand => "expand",
            Self::Collapse => "collapse",
            Self::Select(_) => "select",
            Self::Toggle => "toggle",
            Self::Check => "check",
            Self::Uncheck => "uncheck",
            Self::Scroll(_, _) => "scroll",
            Self::ScrollTo => "scroll-to",
            Self::PressKey(_) => "press",
            Self::KeyDown(_) => "key-down",
            Self::KeyUp(_) => "key-up",
            Self::TypeText(_) => "type",
            Self::Clear => "clear",
            Self::Hover => "hover",
            Self::Drag(_) => "drag",
        }
    }

    pub fn requires_cursor_policy(&self) -> bool {
        matches!(self, Self::Hover | Self::Drag(_))
    }

    pub fn requires_hit_test(&self) -> bool {
        matches!(
            self,
            Self::Click
                | Self::DoubleClick
                | Self::RightClick
                | Self::TripleClick
                | Self::Hover
                | Self::Drag(_)
        )
    }

    pub fn requires_scroll_into_view(&self) -> bool {
        matches!(
            self,
            Self::Click
                | Self::DoubleClick
                | Self::RightClick
                | Self::TripleClick
                | Self::SetValue(_)
                | Self::Expand
                | Self::Collapse
                | Self::Select(_)
                | Self::Toggle
                | Self::Check
                | Self::Uncheck
                | Self::TypeText(_)
                | Self::Clear
                | Self::Hover
                | Self::Drag(_)
        )
    }

    pub fn may_use_focus_fallback(&self) -> bool {
        matches!(self, Self::TypeText(_) | Self::PressKey(_))
    }

    /// Returns the least-permissive interaction policy that can execute this
    /// action with the same fallback behavior as its command entrypoint.
    pub fn base_interaction_policy(&self) -> crate::interaction_policy::InteractionPolicy {
        if self.may_use_focus_fallback() {
            crate::interaction_policy::InteractionPolicy::focus_fallback()
        } else {
            crate::interaction_policy::InteractionPolicy::headless()
        }
    }
}

#[cfg(test)]
#[path = "action_tests.rs"]
mod tests;
