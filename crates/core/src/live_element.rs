use crate::{element_state::ElementState, node::Rect};

#[derive(Debug, Clone, Default)]
pub struct LiveElement {
    pub state: Option<ElementState>,
    pub bounds: Option<Rect>,
    pub available_actions: Option<Vec<String>>,
}
