#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::Deadline;

    use crate::tree::AXElement;

    const SETTLE_POLL_MS: u64 = 40;
    const SETTLE_BUDGET_MS: u64 = 400;

    /// The readback for `perform`, which has no written attribute to re-read.
    #[derive(Default)]
    pub(crate) struct FocusState {
        window_title: Option<String>,
        focused_element: Option<AXElement>,
    }

    /// Accessibility hands back a fresh reference for the same element on every
    /// read, and reuses a released one for a different element later, so a raw
    /// pointer answers neither question this comparison asks. `CFEqual` is the
    /// identity the framework defines.
    impl PartialEq for FocusState {
        fn eq(&self, other: &Self) -> bool {
            self.window_title == other.window_title
                && match (&self.focused_element, &other.focused_element) {
                    (None, None) => true,
                    (Some(mine), Some(theirs)) => crate::tree::same_element(mine, theirs),
                    _ => false,
                }
        }
    }

    pub(crate) fn focus_state(element: &AXElement, deadline: Deadline) -> FocusState {
        let Some(app) = crate::system::app_ops::pid_from_element(element, deadline)
            .map(crate::tree::element_for_pid)
        else {
            return FocusState::default();
        };
        FocusState {
            window_title: read_element(&app, "AXFocusedWindow", deadline).and_then(|window| {
                crate::tree::attributes::copy_string_attr_result(&window, "AXTitle", deadline)
                    .ok()
                    .flatten()
            }),
            focused_element: read_element(&app, "AXFocusedUIElement", deadline),
        }
    }

    pub(crate) fn changed_now(
        before: &FocusState,
        element: &AXElement,
        deadline: Deadline,
    ) -> bool {
        focus_state(element, deadline) != *before
    }

    pub(crate) fn settled_change(
        before: &FocusState,
        element: &AXElement,
        deadline: Deadline,
    ) -> bool {
        let started = std::time::Instant::now();
        loop {
            if focus_state(element, deadline) != *before {
                return true;
            }
            if started.elapsed() >= std::time::Duration::from_millis(SETTLE_BUDGET_MS)
                || deadline.is_expired()
            {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(SETTLE_POLL_MS));
        }
    }

    fn read_element(app: &AXElement, attr: &str, deadline: Deadline) -> Option<AXElement> {
        crate::tree::attributes::copy_element_attr_result(app, attr, deadline)
            .ok()
            .flatten()
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
