use super::ref_owner_observation::RefOwnerObservation;
use crate::system::process_state::ProcessState;
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

pub(crate) fn resolve_ref_owner(
    adapter: &dyn PlatformAdapter,
    entry: &crate::RefEntry,
    deadline: Deadline,
) -> Result<RefOwnerObservation, AppError> {
    let process = entry.process.identity().ok_or_else(|| {
        AdapterError::new(
            crate::ErrorCode::ActionNotSupported,
            "Ref has no process-instance identity for post-action observation",
        )
    })?;
    if adapter.is_application_surface(entry.source.source_surface) {
        return Ok(match adapter.process_state(process, deadline)? {
            ProcessState::Running => RefOwnerObservation::Present(SnapshotOwner::Application(
                application_from_ref(entry)?,
            )),
            state @ (ProcessState::Exited { .. } | ProcessState::Crashed { .. }) => {
                RefOwnerObservation::Absent(Some(state))
            }
            ProcessState::Unresponsive => {
                RefOwnerObservation::Unavailable(ProcessState::Unresponsive)
            }
        });
    }
    let expected = crate::WindowInfo {
        id: entry.source.source_window_id.clone().ok_or_else(|| {
            AdapterError::new(
                crate::ErrorCode::ActionNotSupported,
                "Ref has no source window for post-action observation",
            )
        })?,
        title: entry.source.source_window_title.clone().unwrap_or_default(),
        app: entry.source.source_app.clone().unwrap_or_default(),
        pid: entry.process.pid,
        process_instance: Some(process.instance.clone()),
        bounds: None,
        state: Default::default(),
    };
    match adapter.resolve_window_strict(&expected, deadline) {
        Ok(window) => {
            if window.id != expected.id
                || window.pid != expected.pid
                || window.process_instance != expected.process_instance
            {
                return Err(AdapterError::stale_ref_because(
                    "Original window identity changed during observation",
                )
                .into());
            }
            Ok(RefOwnerObservation::Present(SnapshotOwner::Window(window)))
        }
        Err(error) if error.code == crate::ErrorCode::WindowNotFound => {
            let state = match adapter.process_state(process, deadline) {
                Ok(state) => state,
                Err(error) if error.is_default_not_supported("process_state") => {
                    return Ok(RefOwnerObservation::Absent(None));
                }
                Err(error) => return Err(error.into()),
            };
            Ok(RefOwnerObservation::Absent(match state {
                ProcessState::Exited { .. } | ProcessState::Crashed { .. } => Some(state),
                ProcessState::Running | ProcessState::Unresponsive => None,
            }))
        }
        Err(error) => Err(error.into()),
    }
}

pub(crate) fn application_from_ref(entry: &crate::RefEntry) -> Result<crate::AppInfo, AppError> {
    Ok(crate::AppInfo {
        name: entry.source.source_app.clone().ok_or_else(|| {
            AdapterError::stale_ref_because("root ref has no application identity")
        })?,
        pid: entry.process.pid,
        process_instance: entry.process.process_instance.clone(),
        bundle_id: None,
        presentation: None,
    })
}

pub(crate) fn ref_observation_surface(
    adapter: &dyn PlatformAdapter,
    entry: &crate::RefEntry,
) -> SnapshotSurface {
    if adapter.is_application_surface(entry.source.source_surface) {
        entry.source.source_surface
    } else {
        SnapshotSurface::Window
    }
}

pub(crate) fn is_confirmed_surface_absence(error: &AppError, surface: SnapshotSurface) -> bool {
    matches!(error, AppError::Adapter(error) if error.is_confirmed_surface_absence(surface))
}
