use crate::{AdapterError, AppError, Deadline, PlatformAdapter, SnapshotOwner, SnapshotSurface};

pub(crate) fn resolve_owners(
    adapter: &dyn PlatformAdapter,
    app_name: Option<&str>,
    window_id: Option<&str>,
    surface: SnapshotSurface,
    deadline: Deadline,
) -> Result<Vec<SnapshotOwner>, AppError> {
    if !adapter.is_application_surface(surface) {
        return super::surface_windows(adapter, app_name, window_id, surface, deadline)
            .map(|windows| windows.into_iter().map(SnapshotOwner::Window).collect());
    }
    let app = if window_id.is_some() {
        let window = super::resolve_window(adapter, app_name, window_id, deadline)?;
        crate::system::app_lookup::resolve_app_owning(&window, adapter, deadline)?
    } else if app_name.is_some() {
        crate::system::app_lookup::resolve_app(app_name, adapter, deadline)?
    } else {
        adapter.focused_app(deadline)?.ok_or_else(|| {
            AdapterError::new(
                crate::ErrorCode::AppNotFound,
                "No focused application was found when --app was omitted",
            )
            .unsent()
        })?
    };
    crate::system::app_lookup::process_identity(&app)?;
    Ok(vec![SnapshotOwner::Application(app)])
}
