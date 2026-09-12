#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::Deadline;

    use crate::tree::AXElement;

    const SETTLE_POLL_MS: u64 = 40;
    const SETTLE_BUDGET_MS: u64 = 400;

    /// The readback for `perform`, which has no written attribute to re-read.
    #[derive(Default)]
    pub(crate) struct FocusState {
        focused_element: Option<AXElement>,
    }

    /// Accessibility hands back a fresh reference for the same element on every
    /// read, and reuses a released one for a different element later, so a raw
    /// pointer answers neither question this comparison asks. `CFEqual` is the
    /// identity the framework defines.
    impl PartialEq for FocusState {
        fn eq(&self, other: &Self) -> bool {
            match (&self.focused_element, &other.focused_element) {
                (None, None) => true,
                (Some(mine), Some(theirs)) => crate::tree::same_element(mine, theirs),
                _ => false,
            }
        }
    }

    pub(crate) fn focus_state(element: &AXElement, deadline: Deadline) -> Option<FocusState> {
        let app = crate::system::app_ops::pid_from_element(element, deadline)
            .map(crate::tree::element_for_pid)?;
        Some(FocusState {
            focused_element: crate::tree::attributes::copy_element_attr_result(
                &app,
                "AXFocusedUIElement",
                deadline,
            )
            .ok()?,
        })
    }

    pub(crate) fn changed_now(
        before: &Option<FocusState>,
        element: &AXElement,
        deadline: Deadline,
    ) -> bool {
        let after = focus_state(element, deadline);
        let target_focused = after
            .as_ref()
            .and_then(|state| state.focused_element.as_ref())
            .is_some_and(|focused| crate::tree::same_element(focused, element));
        observed_change(before, &after, target_focused)
    }

    pub(crate) fn settled_change(
        before: &Option<FocusState>,
        element: &AXElement,
        deadline: Deadline,
    ) -> bool {
        if before.is_none() {
            return false;
        }
        let deadline = deadline.capped(std::time::Duration::from_millis(SETTLE_BUDGET_MS));
        while !deadline.is_expired() {
            if changed_now(before, element, deadline) {
                return true;
            }
            std::thread::sleep(
                deadline
                    .remaining()
                    .min(std::time::Duration::from_millis(SETTLE_POLL_MS)),
            );
        }
        false
    }

    fn observed_change(
        before: &Option<FocusState>,
        after: &Option<FocusState>,
        target_focused: bool,
    ) -> bool {
        target_focused && matches!((before, after), (Some(before), Some(after)) if before != after)
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn focus_changes_must_correlate_with_the_target() {
            let before = Some(FocusState {
                focused_element: Some(crate::tree::element_for_pid(1)),
            });
            let after = Some(FocusState {
                focused_element: Some(crate::tree::element_for_pid(2)),
            });
            assert!(!observed_change(&before, &after, false));
            assert!(observed_change(&before, &after, true));
            assert!(!observed_change(&after, &after, true));
        }

        #[test]
        fn missing_observation_cannot_verify_an_action() {
            let before = Some(FocusState::default());
            let after = Some(FocusState::default());
            assert!(!observed_change(&before, &None, true));
            assert!(!observed_change(&None, &after, true));
            assert!(!observed_change(&None, &None, true));
            assert!(!observed_change(&before, &before, true));
            assert!(!observed_change(&before, &after, false));
            assert!(!observed_change(&before, &after, true));
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use agent_desktop_core::Deadline;

    use crate::tree::AXElement;

    #[derive(Default, PartialEq)]
    pub(crate) struct FocusState;

    pub(crate) fn focus_state(_element: &AXElement, _deadline: Deadline) -> FocusState {
        FocusState
    }

    pub(crate) fn changed_now(
        _before: &FocusState,
        _element: &AXElement,
        _deadline: Deadline,
    ) -> bool {
        false
    }

    pub(crate) fn settled_change(
        _before: &FocusState,
        _element: &AXElement,
        _deadline: Deadline,
    ) -> bool {
        false
    }
}

pub(crate) use imp::{changed_now, focus_state, settled_change};
