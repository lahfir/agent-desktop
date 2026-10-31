use crate::{AppInfo, WindowInfo, adapter::native_handle::NativeHandle, refs::RefEntry};

#[derive(Clone, Copy)]
pub enum ObservationRoot<'a> {
    Application {
        app: &'a AppInfo,
        surface: crate::SnapshotSurface,
    },
    Window(&'a WindowInfo),
    Element {
        handle: &'a NativeHandle,
        entry: &'a RefEntry,
        root_ref: Option<&'a str>,
    },
}

impl ObservationRoot<'_> {
    pub fn surface(self) -> crate::SnapshotSurface {
        match self {
            Self::Application { surface, .. } => surface,
            Self::Window(_) => crate::SnapshotSurface::Window,
            Self::Element { entry, .. } => entry.source.source_surface,
        }
    }
}
