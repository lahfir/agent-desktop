use serde::{Deserialize, Serialize};

#[non_exhaustive]
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

    pub fn semantic_capabilities(&self) -> &'static [&'static str] {
        match self {
            Self::Click | Self::DoubleClick | Self::TripleClick => &["Click"],
            Self::RightClick => &["RightClick", "Click"],
            Self::SetValue(_) | Self::Clear => &["SetValue"],
            Self::SetFocus => &["SetFocus"],
            Self::Expand => &["Expand"],
            Self::Collapse => &["Collapse"],
            Self::Select(_) => &["Select", "Click"],
            Self::Toggle => &["Toggle", "Click"],
            Self::Check | Self::Uncheck => &["Toggle", "Click"],
            Self::Scroll(_, _) => &["Scroll", "ScrollTo"],
            Self::ScrollTo => &["ScrollTo"],
            Self::PressKey(_) => &["PressKey"],
            Self::KeyDown(_) => &["KeyDown"],
            Self::KeyUp(_) => &["KeyUp"],
            Self::TypeText(_) => &["TypeText", "SetValue"],
            Self::Hover => &["Hover"],
            Self::Drag(_) => &["Drag"],
        }
    }

    pub fn requires_cursor_policy(&self) -> bool {
        matches!(self, Self::Hover | Self::Drag(_))
    }

    pub fn may_use_focus_fallback(&self) -> bool {
        matches!(self, Self::TypeText(_) | Self::PressKey(_))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub action: Action,
    pub policy: InteractionPolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionPolicy {
    pub allow_focus_steal: bool,
    pub allow_cursor_move: bool,
}

impl ActionRequest {
    pub fn headless(action: Action) -> Self {
        Self {
            action,
            policy: InteractionPolicy::headless(),
        }
    }

    pub fn focus_fallback(action: Action) -> Self {
        Self {
            action,
            policy: InteractionPolicy::focus_fallback(),
        }
    }

    pub fn physical(action: Action) -> Self {
        Self {
            action,
            policy: InteractionPolicy::physical(),
        }
    }
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

#[cfg(test)]
mod policy_tests {
    use super::*;

    #[test]
    fn default_policy_is_headless() {
        let policy = InteractionPolicy::default();
        assert!(!policy.allow_focus_steal);
        assert!(!policy.allow_cursor_move);
    }

    #[test]
    fn headless_request_blocks_physical_side_effects() {
        let request = ActionRequest::headless(Action::Click);
        assert_eq!(request.policy, InteractionPolicy::headless());
    }

    #[test]
    fn focus_fallback_policy_never_moves_cursor() {
        let request = ActionRequest::focus_fallback(Action::Scroll(Direction::Down, 1));
        assert!(request.policy.allow_focus_steal);
        assert!(!request.policy.allow_cursor_move);
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_state: Option<ElementState>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub steps: Vec<ActionStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementState {
    pub role: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub states: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    pub label: String,
    pub outcome: ActionStepOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStepOutcome {
    Attempted,
    Skipped,
    Succeeded,
}

impl ActionStep {
    pub fn attempted(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            outcome: ActionStepOutcome::Attempted,
        }
    }

    pub fn skipped(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            outcome: ActionStepOutcome::Skipped,
        }
    }

    pub fn succeeded(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            outcome: ActionStepOutcome::Succeeded,
        }
    }
}

impl ActionResult {
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            ref_id: None,
            post_state: None,
            steps: Vec::new(),
        }
    }

    pub fn with_ref(mut self, ref_id: impl Into<String>) -> Self {
        self.ref_id = Some(ref_id.into());
        self
    }

    pub fn with_state(mut self, state: ElementState) -> Self {
        self.post_state = Some(state);
        self
    }

    pub fn with_steps(mut self, steps: Vec<ActionStep>) -> Self {
        self.steps = steps;
        self
    }
}
