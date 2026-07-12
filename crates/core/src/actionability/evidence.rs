use crate::{Rect, element_state::ElementState};

pub(super) struct ActionabilityEvidence {
    pub(super) state: ElementState,
    pub(super) states_complete: bool,
    pub(super) bounds: Option<Rect>,
    pub(super) available_actions: Vec<String>,
}
