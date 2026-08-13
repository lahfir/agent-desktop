pub(crate) const SELECTED: &str = "AXSelected";

/// Kept narrow so the settability probe never runs on generic containers.
pub(crate) fn role_activates_by_selection(role: &str) -> bool {
    matches!(
        role,
        "row" | "treeitem" | "cell" | "listitem" | "option" | "tab"
    )
}

#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};
    use core_foundation::{array::CFArray, base::TCFType};

    use crate::actions::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;

    use super::SELECTED;

    /// `AXOutline` and `AXTable` make `AXSelectedRows` writable; `AXList` and
    /// `AXBrowser` columns make `AXSelectedChildren` writable. Most elements
    /// publish both names and accept writes to only one.
    const SELECTION_ATTRIBUTES: [&str; 2] = ["AXSelectedRows", "AXSelectedChildren"];
    const MAX_ANCESTOR_WALK: usize = 6;
    const MAX_SELECTION_READBACK: usize = 64;

    /// Climbs from the target because the clickable label is usually a
    /// descendant of the selectable row. Every write is verified by readback.
    pub(crate) fn select_within_container(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        let mut member = element.clone();
        for _ in 0..MAX_ANCESTOR_WALK {
            if select_member_directly(&member, deadline)? {
                return Ok(DeliveryOutcome::DeliveredVerified);
            }
            let Some(parent) = parent_of(&member, deadline) else {
                return Ok(DeliveryOutcome::NotDelivered);
            };
            if select_member_in_container(&parent, &member, deadline)? {
                return Ok(DeliveryOutcome::DeliveredVerified);
            }
            member = parent;
        }
        Ok(DeliveryOutcome::NotDelivered)
    }

    fn select_member_directly(
        member: &AXElement,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        if !crate::actions::ax_helpers::is_attr_settable(member, SELECTED, deadline)? {
            return Ok(false);
        }
        crate::actions::ax_helpers::set_ax_bool_or_err(member, SELECTED, true, deadline)?;
        Ok(crate::tree::attributes::copy_bool_attr(member, SELECTED, deadline) == Some(true))
    }

    fn select_member_in_container(
        container: &AXElement,
        member: &AXElement,
        deadline: Deadline,
    ) -> Result<bool, AdapterError> {
        for attribute in SELECTION_ATTRIBUTES {
            if !crate::actions::ax_helpers::is_attr_settable(container, attribute, deadline)? {
                continue;
            }
            let members = selection_array(member);
            let cf_attr = core_foundation::string::CFString::new(attribute);
            let error = crate::tree::ax_ipc::set_attribute_value(
                container,
                cf_attr.as_concrete_TypeRef(),
                members.as_CFTypeRef(),
                deadline,
            )?;
            if error == accessibility_sys::kAXErrorSuccess
                && holds_selection(container, member, attribute, deadline)
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// `AXElement` does not implement `TCFType`, so the array is built through
    /// the CoreFoundation C API.
    fn selection_array(member: &AXElement) -> CFArray {
        let values = [member.0 as *const std::ffi::c_void];
        unsafe {
            let raw = core_foundation_sys::array::CFArrayCreate(
                std::ptr::null(),
                values.as_ptr(),
                1,
                &core_foundation_sys::array::kCFTypeArrayCallBacks,
            );
            CFArray::wrap_under_create_rule(raw)
        }
    }

    fn parent_of(element: &AXElement, deadline: Deadline) -> Option<AXElement> {
        crate::tree::attributes::copy_element_attr_result(element, "AXParent", deadline)
            .ok()
            .flatten()
    }

    fn holds_selection(
        container: &AXElement,
        member: &AXElement,
        attribute: &str,
        deadline: Deadline,
    ) -> bool {
        crate::tree::attributes::copy_ax_array_prefix_result(
            container,
            attribute,
            MAX_SELECTION_READBACK,
            deadline,
        )
        .ok()
        .flatten()
        .is_some_and(|selected| {
            selected
                .iter()
                .any(|entry| crate::tree::capabilities::same_element(entry, member))
        })
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::actions::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;

    pub(crate) fn select_within_container(
        _element: &AXElement,
        _deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        Ok(DeliveryOutcome::NotDelivered)
    }
}

pub(crate) use imp::select_within_container;
