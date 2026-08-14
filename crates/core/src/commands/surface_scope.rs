use serde_json::json;

use crate::{AppError, SnapshotSurface, adapter::PlatformAdapter};

/// A ref already carries the surface it was captured from, so naming a second
/// one asks for two roots at once.
pub(crate) fn reject_root_with_surface(
    command: &str,
    root: Option<&str>,
    surface: SnapshotSurface,
) -> Result<(), AppError> {
    if root.is_none() || matches!(surface, SnapshotSurface::Window) {
        return Ok(());
    }
    Err(AppError::invalid_input_with_suggestion(
        "--root cannot be combined with --surface",
        format!(
            "A ref is already scoped to its own surface. Run `{command} --surface` without \
             --root, or drop --surface."
        ),
    ))
}

/// Every surface, including the window, requires the adapter to declare it. An
/// adapter that declares none is unimplemented and must fail closed rather than
/// observe through a partially wired platform.
pub(crate) fn require_supported(
    requested: SnapshotSurface,
    adapter: &dyn PlatformAdapter,
) -> Result<(), AppError> {
    let supported = adapter.supported_surfaces();
    if supported.contains(&requested) {
        return Ok(());
    }
    let supported = supported
        .into_iter()
        .map(SnapshotSurface::as_str)
        .collect::<Vec<_>>();
    Err(crate::AdapterError::new(
        crate::ErrorCode::PlatformNotSupported,
        format!(
            "Snapshot surface '{}' is not supported on this platform",
            requested.as_str()
        ),
    )
    .with_details(json!({
        "requested_surface": requested.as_str(),
        "supported_surfaces": supported
    }))
    .with_suggestion("Choose one of the supported snapshot surfaces")
    .into())
}

#[cfg(test)]
#[path = "surface_scope_tests.rs"]
mod tests;
