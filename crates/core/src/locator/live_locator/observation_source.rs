use crate::{AppInfo, WindowInfo, refs::RefEntry};

use super::ObservationRoot;

#[derive(Debug, Clone)]
pub enum ObservationSource {
    Application {
        app: AppInfo,
        surface: crate::SnapshotSurface,
    },
    Window {
        window: WindowInfo,
        /// The surface actually walked. A ref that was found on the menu bar
        /// must record that, or re-resolving it later searches the window and
        /// reports the element missing.
        surface: crate::SnapshotSurface,
    },
    Element {
        entry: Box<RefEntry>,
        root_ref: Option<String>,
    },
}

impl ObservationSource {
    pub fn from_root(root: &ObservationRoot<'_>, surface: crate::SnapshotSurface) -> Self {
        match root {
            ObservationRoot::Application { app, .. } => Self::Application {
                app: (*app).clone(),
                surface,
            },
            ObservationRoot::Window(window) => Self::Window {
                window: (*window).clone(),
                surface,
            },
            ObservationRoot::Element {
                entry, root_ref, ..
            } => Self::Element {
                entry: Box::new((*entry).clone()),
                root_ref: root_ref.map(str::to_string),
            },
        }
    }
}
