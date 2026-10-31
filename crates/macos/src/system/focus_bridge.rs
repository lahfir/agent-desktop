use agent_desktop_core::{AdapterError, DeliverySemantics, ErrorCode};

use crate::system::process::process_identity::ProcessIdentity;

#[repr(C)]
struct ActivationResult {
    status: u8,
    delivery_started: u8,
}

struct ActivationContext {
    identity: ProcessIdentity,
    failure: Option<AdapterError>,
}

#[cfg(target_os = "macos")]
extern "C" fn verify_identity(context: *mut std::ffi::c_void) -> bool {
    let context = unsafe { &mut *context.cast::<ActivationContext>() };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        match context.identity.still_matches() {
            Ok(true) => true,
            Ok(false) => {
                context.failure = Some(AdapterError::stale_ref_because(
                    "Target process instance changed before activation",
                ));
                false
            }
            Err(error) => {
                context.failure = Some(error);
                false
            }
        }
    }))
    .unwrap_or(false)
}

#[cfg(target_os = "macos")]
pub(crate) fn activate(identity: ProcessIdentity) -> Result<bool, AdapterError> {
    let mut context = ActivationContext {
        identity,
        failure: None,
    };
    let result = unsafe {
        agent_desktop_activate_application(
            identity.pid(),
            verify_identity,
            (&mut context as *mut ActivationContext).cast(),
        )
    };
    if let Some(error) = context.failure {
        return Err(error.with_disposition(DeliverySemantics::not_delivered()));
    }
    let disposition = if result.delivery_started != 0 {
        DeliverySemantics::uncertain()
    } else {
        DeliverySemantics::not_delivered()
    };
    match result.status {
        0 => Ok(result.delivery_started != 0),
        1 | 5 => Err(AdapterError::stale_ref_because(
            "Target application changed before activation",
        )
        .with_disposition(disposition)),
        2 => Err(AdapterError::new(
            ErrorCode::ActionFailed,
            "macOS did not accept the application activation request",
        )
        .with_disposition(DeliverySemantics::not_delivered())),
        status => Err(AdapterError::new(
            ErrorCode::AppUnresponsive,
            "The macOS AppKit bridge failed during application activation",
        )
        .with_details(serde_json::json!({
            "kind": "appkit_bridge", "operation": "activation", "status": status,
        }))
        .with_disposition(disposition)),
    }
}

#[cfg(target_os = "macos")]
unsafe extern "C" {
    fn agent_desktop_activate_application(
        pid: i32,
        verify_identity: extern "C" fn(*mut std::ffi::c_void) -> bool,
        context: *mut std::ffi::c_void,
    ) -> ActivationResult;
}
