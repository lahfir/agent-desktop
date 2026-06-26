use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct InteractionPolicy {
    pub allow_focus_steal: bool,
    pub allow_cursor_move: bool,
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

    pub fn headed() -> Self {
        Self {
            allow_focus_steal: true,
            allow_cursor_move: true,
        }
    }

    /// Returns the least-restrictive policy that satisfies both `self` and
    /// `other`. Used by FFI callers that supply an explicit caller policy: the
    /// result is always at least as permissive as the action's CLI base, and
    /// at least as permissive as what the caller requested.
    pub fn join(self, other: InteractionPolicy) -> InteractionPolicy {
        InteractionPolicy {
            allow_focus_steal: self.allow_focus_steal || other.allow_focus_steal,
            allow_cursor_move: self.allow_cursor_move || other.allow_cursor_move,
        }
    }
}

impl Default for InteractionPolicy {
    fn default() -> Self {
        Self::headless()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_headless_with_headless_stays_headless() {
        assert_eq!(
            InteractionPolicy::headless().join(InteractionPolicy::headless()),
            InteractionPolicy::headless()
        );
    }

    #[test]
    fn join_headless_with_focus_fallback_gives_focus_fallback() {
        assert_eq!(
            InteractionPolicy::headless().join(InteractionPolicy::focus_fallback()),
            InteractionPolicy::focus_fallback()
        );
    }

    #[test]
    fn join_focus_fallback_with_headless_gives_focus_fallback() {
        assert_eq!(
            InteractionPolicy::focus_fallback().join(InteractionPolicy::headless()),
            InteractionPolicy::focus_fallback()
        );
    }

    #[test]
    fn join_headless_with_headed_gives_headed() {
        assert_eq!(
            InteractionPolicy::headless().join(InteractionPolicy::headed()),
            InteractionPolicy::headed()
        );
    }

    #[test]
    fn join_focus_fallback_with_headed_gives_headed() {
        assert_eq!(
            InteractionPolicy::focus_fallback().join(InteractionPolicy::headed()),
            InteractionPolicy::headed()
        );
    }

    #[test]
    fn join_headed_with_headless_gives_headed() {
        assert_eq!(
            InteractionPolicy::headed().join(InteractionPolicy::headless()),
            InteractionPolicy::headed()
        );
    }
}
