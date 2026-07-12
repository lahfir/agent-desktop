use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, ErrorCode, ProcessIdentity};

pub(crate) fn activate(process: ProcessIdentity, deadline: Deadline) -> Result<(), AdapterError> {
    let identity =
        crate::system::process_identity::require_core(&process).map_err(not_delivered)?;
    let application = crate::tree::element_for_pid(identity.pid());
    let current = crate::tree::surface_read::boolean(
        &application,
        "AXManualAccessibility",
        instant(deadline).map_err(not_delivered)?,
    )
    .map_err(not_delivered)?;
    if current == Some(true) {
        return Ok(());
    }
    set_manual_accessibility(&application, deadline)
}

fn set_manual_accessibility(
    application: &crate::tree::AXElement,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    use core_foundation::{base::TCFType, boolean::CFBoolean, string::CFString};

    let attribute = CFString::new("AXManualAccessibility");
    let error = crate::tree::ax_ipc::set_attribute_value(
        application,
        attribute.as_concrete_TypeRef(),
        CFBoolean::true_value().as_CFTypeRef(),
        deadline,
    )?;
    let delivered = crate::actions::ax_mutation::classify_result(
        application,
        "AXManualAccessibility",
        "AXUIElementSetAttributeValue",
        error,
    )?;
    if !delivered {
        return Err(AdapterError::new(
            ErrorCode::ActionNotSupported,
            "Renderer does not support AXManualAccessibility activation",
        )
        .with_disposition(DeliverySemantics::not_delivered()));
    }
    if deadline.is_expired() {
        return Err(deadline
            .timeout_error()
            .with_disposition(DeliverySemantics::delivered_unverified()));
    }
    Ok(())
}

fn instant(deadline: Deadline) -> Result<std::time::Instant, AdapterError> {
    std::time::Instant::now()
        .checked_add(deadline.remaining_slice(std::time::Duration::from_millis(250))?)
        .ok_or_else(|| AdapterError::new(ErrorCode::InvalidArgs, "Deadline is out of range"))
}

fn not_delivered(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::not_delivered())
}
