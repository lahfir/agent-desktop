use super::PointerDelivery;
use crate::action::Action;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ActionabilityRequirements {
    pub(crate) visible: bool,
    pub(crate) stable: bool,
    pub(crate) enabled: bool,
    pub(crate) editable: bool,
    pub(crate) receives_events: bool,
}

impl ActionabilityRequirements {
    pub(crate) fn for_action(action: &Action) -> Self {
        match action {
            Action::Click | Action::DoubleClick | Action::RightClick | Action::TripleClick => {
                Self::new(true, true, true, false, true)
            }
            Action::Hover | Action::Drag(_) => Self::new(true, true, false, false, true),
            Action::SetValue(_) | Action::TypeText(_) | Action::Clear => {
                Self::new(true, false, true, true, false)
            }
            Action::Select(_) => Self::new(true, false, true, false, false),
            Action::Expand
            | Action::Collapse
            | Action::Toggle
            | Action::Check
            | Action::Uncheck
            | Action::Scroll(_, _) => Self::new(true, true, true, false, false),
            Action::ScrollTo => Self::new(false, true, false, false, false),
            Action::SetFocus | Action::PressKey(_) | Action::KeyDown(_) | Action::KeyUp(_) => {
                Self::new(false, false, false, false, false)
            }
        }
    }

    pub(crate) fn pointer_delivery(
        &self,
        action: &Action,
        available_actions: &[String],
        policy: crate::InteractionPolicy,
    ) -> PointerDelivery {
        if !self.receives_events {
            return PointerDelivery::NotApplicable;
        }
        if policy.is_headed() && action.headed_requirement().requires_cursor() {
            PointerDelivery::Physical
        } else if crate::capability::supports_direct_semantic_pointer_delivery(
            action,
            available_actions,
        ) {
            PointerDelivery::Semantic
        } else {
            PointerDelivery::Physical
        }
    }

    /// Whether visibility must be proven from pointer bounds. Strictly headless
    /// semantic delivery (AXPress/AXShowMenu, or an advertised editable-text
    /// capability) does not use a pointer, so closed-menu items and editors
    /// without usable bounds stay actionable. The visibility gate applies this
    /// only to targets that report bounds but no owning window (`offscreen`
    /// unknown), the shape macOS closed-menu items have: a zero-size control
    /// inside a known window, or a target with no bounds at all, still needs
    /// bounds. Explicit
    /// hidden/offscreen state still gates them; headed, focus-fallback, and
    /// physical paths keep the geometry requirement.
    pub(crate) fn requires_geometry(
        &self,
        request: &crate::ActionRequest,
        available_actions: &[String],
    ) -> bool {
        if request.policy != crate::InteractionPolicy::headless() {
            return true;
        }
        let direct_pointer = crate::capability::supports_direct_semantic_pointer_delivery(
            &request.action,
            available_actions,
        );
        let direct_text = self.editable
            && crate::capability::contains_any(
                available_actions,
                crate::capability::for_action(&request.action),
            );
        !(direct_pointer || direct_text)
    }

    pub(crate) fn requires_stability(&self, pointer_delivery: PointerDelivery) -> bool {
        self.stable && !matches!(pointer_delivery, PointerDelivery::Semantic)
    }

    const fn new(
        visible: bool,
        stable: bool,
        enabled: bool,
        editable: bool,
        receives_events: bool,
    ) -> Self {
        Self {
            visible,
            stable,
            enabled,
            editable,
            receives_events,
        }
    }
}

pub(crate) fn requires_stability(action: &Action) -> bool {
    ActionabilityRequirements::for_action(action).stable
}

#[cfg(test)]
#[path = "requirements_tests.rs"]
mod tests;
