use agent_desktop_core::{AdapterError, Deadline, ErrorCode, Point};

use crate::tree::AXElement;

pub(crate) struct PreparedPhysicalTarget {
    identity: crate::system::process::process_identity::ProcessIdentity,
    window: Option<AXElement>,
}

impl PreparedPhysicalTarget {
    pub(crate) fn prepare(element: &AXElement, deadline: Deadline) -> Result<Self, AdapterError> {
        let pid =
            crate::system::app::app_ops::pid_from_element(element, deadline).ok_or_else(|| {
                AdapterError::new(
                    ErrorCode::StaleRef,
                    "Physical input target no longer has an owning application",
                )
            })?;
        let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)?
            .ok_or_else(|| {
                AdapterError::new(
                    ErrorCode::StaleRef,
                    "Physical input target process exited before input preparation",
                )
            })?;
        let window = if crate::tree::native_menu::is_native_menu(element, deadline)? {
            None
        } else {
            Some(crate::system::ref_focus_window::owning_window(
                element, pid, deadline,
            )?)
        };
        crate::system::focus::verify_app_focused(pid, deadline)?;
        if let Some(window) = &window {
            crate::system::focus::verify_window_main(window, deadline)?;
        }
        Ok(Self { identity, window })
    }

    pub(crate) fn verify_pointer(
        &self,
        element: &AXElement,
        point: &Point,
        deadline: Deadline,
    ) -> Result<(), AdapterError> {
        crate::system::focus::verify_app_focused(self.identity.pid(), deadline)?;
        if let Some(window) = &self.window {
            crate::system::focus::verify_window_main(window, deadline)?;
        }
        verify_pointer_identity(
            crate::tree::hit_test::hit_test_ax_element(element, point.clone(), deadline),
            || self.identity.still_matches(),
        )
    }
}

fn verify_pointer_identity(
    hit: Result<agent_desktop_core::HitTestResult, AdapterError>,
    same_instance: impl FnOnce() -> Result<bool, AdapterError>,
) -> Result<(), AdapterError> {
    match hit? {
        agent_desktop_core::adapter::hit_test::HitTestResult::ReachesTarget => {}
        agent_desktop_core::adapter::hit_test::HitTestResult::InterceptedBy {
            role, name, ..
        } => {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Physical input point is intercepted by another accessibility element",
            )
            .with_details(serde_json::json!({
                "physical_delivery_started": false,
                "occluder_role": role,
                "occluder_name": name,
            })));
        }
        agent_desktop_core::adapter::hit_test::HitTestResult::Unknown => {
            return Err(AdapterError::new(
                ErrorCode::ActionFailed,
                "Physical input target could not be proven at the final input point",
            )
            .with_details(serde_json::json!({ "physical_delivery_started": false })));
        }
    }
    if !same_instance()? {
        return Err(AdapterError::new(
            ErrorCode::StaleRef,
            "Physical input target process instance changed at input delivery",
        )
        .with_details(serde_json::json!({ "physical_delivery_started": false })));
    }
    Ok(())
}

#[cfg(test)]
#[path = "physical_target_tests.rs"]
mod tests;
