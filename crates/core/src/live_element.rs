use crate::{LiveIdentity, Rect, element_state::ElementState};

#[derive(Debug, Clone)]
pub struct LiveElement {
    pub identity: LiveIdentity,
    pub state: ElementState,
    pub states_complete: bool,
    pub bounds: Option<Rect>,
    pub available_actions: Vec<String>,
}
