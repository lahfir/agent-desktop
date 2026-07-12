use crate::{WindowInfo, refs::RefEntry};

use super::ObservationRoot;

#[derive(Debug, Clone)]
pub enum ObservationSource {
    Window(WindowInfo),
    Element {
        entry: Box<RefEntry>,
        root_ref: Option<String>,
    },
}

impl ObservationSource {
    pub fn from_root(root: &ObservationRoot<'_>) -> Self {
        match root {
            ObservationRoot::Window(window) => Self::Window((*window).clone()),
            ObservationRoot::Element {
                entry, root_ref, ..
            } => Self::Element {
                entry: Box::new((*entry).clone()),
                root_ref: root_ref.map(str::to_string),
            },
        }
    }
}
