use crate::node::Rect;

pub const FOCUSED: &str = "focused";
pub const DISABLED: &str = "disabled";
pub const SECURE: &str = "secure";
pub const EXPANDED: &str = "expanded";
pub const CHECKED: &str = "checked";
pub const SELECTED: &str = "selected";
pub const HIDDEN: &str = "hidden";
pub const BUSY: &str = "busy";
pub const MODAL: &str = "modal";
pub const REQUIRED: &str = "required";
pub const INDETERMINATE: &str = "indeterminate";
pub const PRESSED: &str = "pressed";
pub const READONLY: &str = "readonly";
pub const OFFSCREEN: &str = "offscreen";
pub const INVALID: &str = "invalid";
pub const MULTISELECTABLE: &str = "multiselectable";
pub const HASPOPUP: &str = "haspopup";

pub const STATE_VOCABULARY: &[&str] = &[
    FOCUSED,
    DISABLED,
    SECURE,
    EXPANDED,
    CHECKED,
    SELECTED,
    HIDDEN,
    BUSY,
    MODAL,
    REQUIRED,
    INDETERMINATE,
    PRESSED,
    READONLY,
    OFFSCREEN,
    INVALID,
    MULTISELECTABLE,
    HASPOPUP,
];

pub fn has_state(states: &[String], token: &str) -> bool {
    states.iter().any(|state| state == token)
}

pub fn assert_states_in_vocabulary(states: &[String]) {
    for state in states {
        assert!(
            STATE_VOCABULARY.contains(&state.as_str()),
            "state token '{state}' is not in STATE_VOCABULARY"
        );
    }
}

pub fn is_visible(bounds: Option<Rect>, states: &[String]) -> bool {
    crate::actionability::bounds_are_visible(bounds)
        && !has_state(states, HIDDEN)
        && !has_state(states, OFFSCREEN)
}

pub struct VisibilityEvidence {
    pub bounds: Option<Rect>,
    pub states: Vec<String>,
    pub bounds_from_live: bool,
    pub states_from_live: bool,
}

impl VisibilityEvidence {
    pub fn applicable(&self) -> bool {
        self.bounds_from_live && self.states_from_live
    }

    pub fn result(&self) -> bool {
        if !self.applicable() {
            return false;
        }
        is_visible(self.bounds, &self.states)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::Rect;

    #[test]
    fn vocabulary_contains_seventeen_tokens() {
        assert_eq!(STATE_VOCABULARY.len(), 17);
    }

    #[test]
    fn hidden_element_is_not_visible() {
        let bounds = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        assert!(!is_visible(bounds, &[HIDDEN.to_string()]));
    }

    #[test]
    fn zero_sized_bounds_are_not_visible() {
        let bounds = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 0.0,
            height: 10.0,
        });
        assert!(!is_visible(bounds, &[]));
    }

    #[test]
    fn offscreen_element_is_not_visible() {
        let bounds = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        assert!(!is_visible(bounds, &[OFFSCREEN.to_string()]));
    }

    #[test]
    fn visible_element_with_live_evidence() {
        let bounds = Some(Rect {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        });
        assert!(is_visible(bounds, &[]));
    }
}
