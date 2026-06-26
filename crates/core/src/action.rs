use serde::{Deserialize, Serialize};

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

    pub fn may_use_focus_fallback(&self) -> bool {
        matches!(self, Self::TypeText(_) | Self::PressKey(_))
    }

    /// Returns the minimum `InteractionPolicy` the CLI uses for this action.
    /// `TypeText` and `PressKey` require focus to land in the right field, so
    /// their base is `focus_fallback`. Everything else is pure-AX and uses
    /// `headless`. FFI callers join this base with their caller-supplied policy
    /// so they can only elevate, never downgrade below CLI parity.
    pub fn base_interaction_policy(&self) -> crate::interaction_policy::InteractionPolicy {
        if self.may_use_focus_fallback() {
            crate::interaction_policy::InteractionPolicy::focus_fallback()
        } else {
            crate::interaction_policy::InteractionPolicy::headless()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DragParams {
    pub from: Point,
    pub to: Point,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Time to hold the dragged item over the destination before releasing.
    /// macOS drop targets often need the drag to dwell over them before they
    /// register as the drop destination; too short and the gesture lands as a
    /// drag with no drop. `None` uses the adapter default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drop_delay_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MouseEventKind {
    Move,
    Down,
    Up,
    Click { count: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MouseEvent {
    pub kind: MouseEventKind,
    pub point: Point,
    pub button: MouseButton,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WindowOp {
    Resize { width: f64, height: f64 },
    Move { x: f64, y: f64 },
    Minimize,
    Maximize,
    Restore,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyCombo {
    pub key: String,
    pub modifiers: Vec<Modifier>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum Modifier {
    Cmd,
    Ctrl,
    Alt,
    Shift,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_names_do_not_include_payloads() {
        let cases = [
            (Action::SetValue("private".into()), "set-value"),
            (Action::Select("private".into()), "select"),
            (Action::TypeText("private".into()), "type"),
            (
                Action::PressKey(KeyCombo {
                    key: "A".into(),
                    modifiers: vec![Modifier::Cmd],
                }),
                "press",
            ),
        ];

        for (action, expected) in cases {
            assert_eq!(action.name(), expected);
        }
    }
}
