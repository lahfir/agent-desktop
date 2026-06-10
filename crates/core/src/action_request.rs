use crate::action::Action;
use crate::interaction_policy::InteractionPolicy;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionRequest {
    pub action: Action,
    pub policy: InteractionPolicy,
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
}
