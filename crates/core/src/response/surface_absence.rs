use crate::{AdapterError, ErrorCode, SnapshotSurface};

impl AdapterError {
    /// Records a complete lookup that proved the requested surface absent.
    pub fn surface_absent(surface: SnapshotSurface, message: &str) -> Self {
        Self::element_not_found(message).with_details(serde_json::json!({
            "kind": "surface_absent",
            "complete": true,
            "surface": surface.as_str(),
        }))
    }

    pub fn is_confirmed_surface_absence(&self, surface: SnapshotSurface) -> bool {
        self.code == ErrorCode::ElementNotFound
            && self.details.as_ref().is_some_and(|details| {
                details["kind"] == "surface_absent"
                    && details["complete"] == true
                    && details["surface"] == surface.as_str()
            })
    }
}
