use crate::action::Action;
use crate::interaction_policy::InteractionPolicy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub action: Action,
    pub policy: InteractionPolicy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<u64>,
}

impl ActionRequest {
    pub fn headless(action: Action) -> Self {
        Self {
            action,
            policy: InteractionPolicy::headless(),
            timeout_ms: None,
        }
    }

    pub fn focus_fallback(action: Action) -> Self {
        Self {
            action,
            policy: InteractionPolicy::focus_fallback(),
            timeout_ms: None,
        }
    }

    pub fn headed(action: Action) -> Self {
        Self {
            action,
            policy: InteractionPolicy::headed(),
            timeout_ms: None,
        }
    }

    pub fn with_timeout_ms(mut self, timeout_ms: Option<u64>) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, Direction};

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

    /// Regression coverage: `ActionRequest.timeout_ms` must stay
    /// `#[serde(default)]` so a legacy payload recorded before `timeout_ms`
    /// existed (or any FFI/batch caller that omits the key) still
    /// deserializes instead of erroring out.
    #[test]
    fn action_request_json_without_timeout_ms_key_deserializes_to_none() {
        let request: ActionRequest = serde_json::from_value(serde_json::json!({
            "action": "Click",
            "policy": {
                "allow_focus_steal": false,
                "allow_cursor_move": false,
            },
        }))
        .unwrap();

        assert_eq!(request.timeout_ms, None);
    }
}
