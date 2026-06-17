use crate::{action_step::ActionStep, element_state::ElementState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_state: Option<ElementState>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub steps: Vec<ActionStep>,
}

impl ActionResult {
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            post_state: None,
            steps: Vec::new(),
        }
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
