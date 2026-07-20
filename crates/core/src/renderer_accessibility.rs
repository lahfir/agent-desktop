use crate::{
    AppError, PlatformAdapter,
    live_locator::{ObservationRequest, ObservationRoot, ObservedTree},
};

pub(crate) fn observe_tree(
    adapter: &dyn PlatformAdapter,
    root: ObservationRoot<'_>,
    request: &ObservationRequest,
) -> Result<ObservedTree, AppError> {
    let mut activated = false;
    loop {
        match adapter.observe_tree(root, request) {
            Ok(tree) => return Ok(tree),
            Err(error) if error.requires_renderer_accessibility_activation() => {
                if !activated {
                    let acquired = adapter.acquire_interaction_lease(request.deadline)?;
                    adapter.activate_renderer_accessibility(root_process(root)?, &acquired)?;
                    drop(acquired);
                    activated = true;
                }
                let remaining = request.deadline.remaining();
                if remaining.is_zero() {
                    return Err(request.deadline.timeout_error().into());
                }
                std::thread::sleep(remaining.min(std::time::Duration::from_millis(25)));
            }
            Err(error) => return Err(AppError::Adapter(error)),
        }
    }
}

fn root_process(root: ObservationRoot<'_>) -> Result<crate::ProcessIdentity, AppError> {
    match root {
        ObservationRoot::Window(window) => window
            .process_instance
            .as_deref()
            .map(|instance| crate::ProcessIdentity::new(window.pid, instance)),
        ObservationRoot::Element { entry, .. } => entry
            .process
            .process_instance
            .as_deref()
            .map(|instance| crate::ProcessIdentity::new(entry.process.pid, instance)),
    }
    .ok_or_else(|| {
        crate::AdapterError::stale_ref("renderer process instance is unavailable").into()
    })
}

#[cfg(test)]
#[path = "renderer_accessibility_tests.rs"]
mod tests;
