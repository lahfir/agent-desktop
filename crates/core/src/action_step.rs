use crate::action_step_outcome::ActionStepOutcome;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionStep {
    pub label: String,
    pub outcome: ActionStepOutcome,
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
