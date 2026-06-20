use crate::action_step_outcome::ActionStepOutcome;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    label: String,
    pub outcome: ActionStepOutcome,
}

impl ActionStep {
    pub fn attempted(label: &'static str) -> Self {
        Self {
            label: label.to_string(),
            outcome: ActionStepOutcome::Attempted,
        }
    }

    pub fn skipped(label: &'static str) -> Self {
        Self {
            label: label.to_string(),
            outcome: ActionStepOutcome::Skipped,
        }
    }

    pub fn succeeded(label: &'static str) -> Self {
        Self {
            label: label.to_string(),
            outcome: ActionStepOutcome::Succeeded,
        }
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}
