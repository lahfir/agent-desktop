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
    /// Time to hold over the destination before releasing. Some platforms require
    /// a minimum dwell before the drop registers; `None` uses the adapter default.
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
    use crate::interaction_policy::InteractionPolicy;

    fn dummy_key() -> KeyCombo {
        KeyCombo {
            key: "a".into(),
            modifiers: vec![],
        }
    }

    fn dummy_drag() -> DragParams {
        DragParams {
            from: Point { x: 0.0, y: 0.0 },
            to: Point { x: 1.0, y: 1.0 },
            duration_ms: None,
            drop_delay_ms: None,
        }
    }

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

    #[test]
    fn pure_ax_actions_base_policy_is_headless() {
        let headless = InteractionPolicy::headless();
        let pure_ax: &[Action] = &[
            Action::Click,
            Action::DoubleClick,
            Action::RightClick,
            Action::TripleClick,
            Action::SetFocus,
            Action::Expand,
            Action::Collapse,
            Action::Toggle,
            Action::Check,
            Action::Uncheck,
            Action::ScrollTo,
            Action::Clear,
            Action::Scroll(Direction::Down, 3),
            Action::SetValue("v".into()),
            Action::Select("s".into()),
        ];
        for action in pure_ax {
            assert_eq!(
                action.base_interaction_policy(),
                headless,
                "{} must use headless base policy",
                action.name()
            );
        }
    }

    #[test]
    fn press_key_and_type_text_base_policy_is_focus_fallback() {
        let focus = InteractionPolicy::focus_fallback();
        assert_eq!(
            Action::PressKey(KeyCombo {
                key: "a".into(),
                modifiers: vec![Modifier::Cmd],
            })
            .base_interaction_policy(),
            focus,
            "PressKey must request focus_fallback to land in the right field"
        );
        assert_eq!(
            Action::TypeText("hello".into()).base_interaction_policy(),
            focus,
            "TypeText must request focus_fallback"
        );
    }

    #[test]
    fn key_down_and_key_up_base_policy_is_headless_unlike_press_key() {
        let headless = InteractionPolicy::headless();
        assert_eq!(
            Action::KeyDown(dummy_key()).base_interaction_policy(),
            headless,
            "KeyDown must be headless; raw key-down events do not need focus theft"
        );
        assert_eq!(
            Action::KeyUp(dummy_key()).base_interaction_policy(),
            headless,
            "KeyUp must be headless"
        );
    }

    #[test]
    fn hover_and_drag_base_policy_is_headless_independent_of_cursor_requirement() {
        let headless = InteractionPolicy::headless();
        assert_eq!(
            Action::Hover.base_interaction_policy(),
            headless,
            "Hover base_interaction_policy is headless even though requires_cursor_policy is true"
        );
        assert_eq!(
            Action::Drag(dummy_drag()).base_interaction_policy(),
            headless,
            "Drag base_interaction_policy is headless even though requires_cursor_policy is true"
        );
        assert!(
            Action::Hover.requires_cursor_policy(),
            "Hover.requires_cursor_policy() must still be true"
        );
        assert!(
            Action::Drag(dummy_drag()).requires_cursor_policy(),
            "Drag.requires_cursor_policy() must still be true"
        );
    }
}
