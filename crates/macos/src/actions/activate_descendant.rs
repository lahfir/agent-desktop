#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::actions::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;

    const MAX_CANDIDATES: usize = 4;

    /// A row often publishes no activation itself while the cell inside it
    /// does: Finder's sidebar `treeitem` is inert and its `cell` carries
    /// `AXOpen`. Without this the chain falls through to writing selection,
    /// which the row accepts and reports back while the application never
    /// navigates.
    ///
    /// Descending is a guess about which child owns the row's behaviour, so
    /// only an observed effect ends the chain here. A child that merely claims
    /// success — Xcode's rows answer that to `AXConfirm` and do nothing — must
    /// not consume the selection step that actually works for them.
    pub(crate) fn activate_descendant(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let instant = crate::tree::locator_deadline::from_operation(deadline)?;
        let children = crate::tree::attributes::copy_ax_array_prefix_result(
            element,
            "AXChildren",
            MAX_CANDIDATES,
            instant,
        )
        .ok()
        .flatten()
        .unwrap_or_default();

        for child in children {
            for action in crate::tree::action_list::PRIMARY_ACTIVATION_ACTIONS {
                let outcome =
                    crate::actions::ax_helpers::perform_observed_action(&child, action, deadline)?;
                if outcome.was_verified() {
                    return Ok(outcome);
                }
            }
        }
        Ok(DeliveryOutcome::NotDelivered)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::actions::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;

    pub(crate) fn activate_descendant(
        _element: &AXElement,
        _deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        Ok(DeliveryOutcome::NotDelivered)
    }
}

pub(crate) use imp::activate_descendant;
