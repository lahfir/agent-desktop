use crate::{DeliverySemantics, action_step::ActionStep, element_state::ElementState};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionResult {
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post_state: Option<ElementState>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub steps: Vec<ActionStep>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    #[serde(
        default = "default_action_disposition",
        deserialize_with = "deserialize_action_disposition"
    )]
    disposition: DeliverySemantics,
}

impl ActionResult {
    pub fn satisfied_without_delivery(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            post_state: None,
            steps: Vec::new(),
            details: None,
            disposition: DeliverySemantics::not_delivered(),
        }
    }

    pub fn delivered_unverified(action: impl Into<String>) -> Self {
        Self {
            action: action.into(),
            post_state: None,
            steps: Vec::new(),
            details: None,
            disposition: default_action_disposition(),
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

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_verified_delivery(mut self) -> Self {
        if self.disposition == DeliverySemantics::delivered_unverified() {
            self.disposition = DeliverySemantics::delivered_verified();
        }
        self
    }

    pub const fn disposition(&self) -> DeliverySemantics {
        self.disposition
    }
}

const fn default_action_disposition() -> DeliverySemantics {
    DeliverySemantics::delivered_unverified()
}

fn deserialize_action_disposition<'de, D>(deserializer: D) -> Result<DeliverySemantics, D::Error>
where
    D: Deserializer<'de>,
{
    let disposition = DeliverySemantics::deserialize(deserializer)?;
    match disposition {
        DeliverySemantics::NotDelivered
        | DeliverySemantics::DeliveredUnverified
        | DeliverySemantics::DeliveredVerified => Ok(disposition),
        _ => Err(serde::de::Error::custom(
            "successful action result must represent satisfied or delivered work",
        )),
    }
}

#[cfg(test)]
#[path = "action_result_tests.rs"]
mod tests;
