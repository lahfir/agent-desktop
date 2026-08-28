#[cfg(target_os = "macos")]
mod imp {
    use crate::{
        cf_type::created_cf_array,
        tree::{
            NodeAttrs,
            ax_element::AXElement,
            ax_value,
            element_bounds::rect_from_parts,
            node_attr_states::NodeAttrStates,
            node_attribute_decode,
            node_attribute_names::copy_node_attribute_values,
            node_attribute_read::NodeAttributeRead,
            node_attribute_status::{
                AX_DOM_IDENTIFIER, AX_IDENTIFIER, LABEL, NodeAttributeStatus, ROLE, SUBROLE, VALUE,
            },
            node_attrs::{parse_bool_attr, parse_enabled},
            node_control_states::NodeControlStates,
            node_identifiers::NodeIdentifiers,
            node_semantic_states::NodeSemanticStates,
        },
    };
    use accessibility_sys::{
        kAXDescriptionAttribute, kAXErrorFailure, kAXErrorSuccess, kAXRoleAttribute,
        kAXSubroleAttribute, kAXTitleAttribute,
    };
    use agent_desktop_core::EvidenceRequirements;

    pub(crate) fn fetch_node_attrs_with_status_for(
        element: &AXElement,
        requirements: EvidenceRequirements,
        deadline: std::time::Instant,
        usage: &mut crate::tree::observation_usage::ObservationUsage,
    ) -> NodeAttributeRead {
        let mask = crate::tree::node_attribute_names::safe_attribute_mask(requirements);
        let requested =
            crate::tree::node_attribute_names::requested_indices(mask).collect::<Vec<_>>();
        let mut requested_count = requested.len() as u64;
        if crate::tree::locator_deadline::prepare(element, deadline).is_err() {
            return failed_node_attribute_read(
                accessibility_sys::kAXErrorCannotComplete,
                requested_count,
                true,
                1,
            );
        }
        let (batch_error, batch_result) = copy_node_attribute_values(element, mask, deadline);
        if batch_error != kAXErrorSuccess || batch_result.is_null() {
            if !batch_result.is_null() {
                drop(created_cf_array(batch_result));
            }
            return failed_node_attribute_read(
                if batch_error == kAXErrorSuccess {
                    kAXErrorFailure
                } else {
                    batch_error
                },
                requested_count,
                false,
                1,
            );
        }
        let Some(attributes) = created_cf_array(batch_result) else {
            return failed_node_attribute_read(kAXErrorFailure, requested_count, false, 1);
        };
        let mut batch_reads = 1;
        let fallback_reads = 0;

        let mut texts: [Option<String>; 17] = Default::default();
        let mut labelled_by_text = None;
        let mut position = None;
        let mut size = None;
        let mut has_scrollbars = false;
        let mut subrole = None;
        let mut status = NodeAttributeStatus::default();
        let mut deadline_exhausted = false;
        for (index, item) in requested.into_iter().zip(attributes.into_iter()) {
            if node_attribute_decode::is_null(&item) {
                continue;
            }
            if let Some(error) = node_attribute_decode::slot_error(&item) {
                status.record_slot_error(index, error);
                continue;
            }
            match index {
                0..=16 => match node_attribute_decode::text(index, &item, usage) {
                    Some(value) if value.complete => texts[index] = Some(value.value),
                    Some(_) => status.record_truncated(index),
                    None if index == LABEL && node_attribute_decode::is_number(&item) => {}
                    None => status.record_slot_error(index, kAXErrorFailure),
                },
                17 => {
                    if let Some(label) = ax_value::retained_ax_element(&item) {
                        match title_ui_element_text(&label, deadline, usage) {
                            Ok(Some(value)) if value.complete => {
                                labelled_by_text = Some(value.value)
                            }
                            Ok(Some(_)) => status.record_truncated(index),
                            Ok(None) => {}
                            Err(error) => {
                                deadline_exhausted |=
                                    error == accessibility_sys::kAXErrorCannotComplete;
                                status.record_slot_error(index, error);
                            }
                        }
                    } else {
                        status.record_slot_error(index, kAXErrorFailure);
                    }
                }
                18 => match node_attribute_decode::point(&item) {
                    Some(value) => position = Some(value),
                    None => status.record_slot_error(index, kAXErrorFailure),
                },
                19 => match node_attribute_decode::size(&item) {
                    Some(value) => size = Some(value),
                    None => status.record_slot_error(index, kAXErrorFailure),
                },
                20 | 21 => {
                    if ax_value::retained_ax_element(&item).is_some() {
                        has_scrollbars = true;
                    } else {
                        status.record_slot_error(index, kAXErrorFailure);
                    }
                }
                22 => match node_attribute_decode::text(index, &item, usage) {
                    Some(value) if value.complete => subrole = Some(value.value),
                    Some(_) => status.record_truncated(index),
                    None => status.record_slot_error(index, kAXErrorFailure),
                },
                _ => {}
            }
        }
        let get = |index: usize| texts.get(index).and_then(Clone::clone);
        let role = get(ROLE);
        if status.field_unknown(SUBROLE) {
            subrole = None;
        }
        let role_complete = !status.field_unknown(ROLE) && role.is_some();
        let subrole_complete = !status.field_unknown(SUBROLE);
        if crate::tree::node_attribute_names::should_read_value(
            requirements,
            role.as_deref(),
            subrole.as_deref(),
            role_complete,
            subrole_complete,
        ) {
            batch_reads += 1;
            requested_count += 1;
            if crate::tree::locator_deadline::prepare(element, deadline).is_err() {
                deadline_exhausted = true;
                status.record_slot_error(VALUE, accessibility_sys::kAXErrorCannotComplete);
            } else {
                let value_mask = crate::tree::node_attribute_status::attribute_bit(VALUE);
                let (value_error, value_result) =
                    copy_node_attribute_values(element, value_mask, deadline);
                if value_error != kAXErrorSuccess || value_result.is_null() {
                    if !value_result.is_null() {
                        drop(created_cf_array(value_result));
                    }
                    status.record_slot_error(
                        VALUE,
                        if value_error == kAXErrorSuccess {
                            kAXErrorFailure
                        } else {
                            value_error
                        },
                    );
                } else if let Some(values) = created_cf_array(value_result) {
                    match values.into_iter().next() {
                        Some(item) if node_attribute_decode::is_null(&item) => {}
                        Some(item) => {
                            if let Some(error) = node_attribute_decode::slot_error(&item) {
                                status.record_slot_error(VALUE, error);
                            } else {
                                match node_attribute_decode::text(VALUE, &item, usage) {
                                    Some(value) if value.complete => {
                                        texts[VALUE] = Some(value.value)
                                    }
                                    Some(_) => status.record_truncated(VALUE),
                                    None => status.record_slot_error(VALUE, kAXErrorFailure),
                                }
                            }
                        }
                        None => status.record_slot_error(VALUE, kAXErrorFailure),
                    }
                } else {
                    status.record_slot_error(VALUE, kAXErrorFailure);
                }
            }
        } else if requirements.value && (!role_complete || !subrole_complete) {
            status.record_slot_error(VALUE, accessibility_sys::kAXErrorCannotComplete);
        }
        let get = |index: usize| texts.get(index).and_then(Clone::clone);
        let value = get(VALUE);
        let readonly = if requirements.states {
            crate::tree::readonly::read_readonly(element, role.as_deref(), deadline)
        } else {
            crate::tree::readonly::ReadonlyRead {
                value: None,
                error: None,
                attempted: false,
                deadline_exhausted: false,
            }
        };
        if let Some(error) = readonly.error {
            status.record_readonly_error(error);
        }
        deadline_exhausted |= readonly.deadline_exhausted;
        let identifier_field = |index| {
            if status.field_unknown(index) {
                agent_desktop_core::LocatorField::Unknown
            } else {
                get(index)
                    .map(agent_desktop_core::LocatorField::Known)
                    .unwrap_or(agent_desktop_core::LocatorField::Absent)
            }
        };
        NodeAttributeRead {
            attrs: NodeAttrs {
                name_evidence: agent_desktop_core::NameEvidence {
                    explicit_label: get(LABEL).or_else(|| {
                        crate::tree::roles::accessible_name_from_subrole(subrole.as_deref())
                            .map(str::to_owned)
                    }),
                    labelled_by_text,
                    native_title: get(1),
                    static_value: (role.as_deref() == Some("AXStaticText"))
                        .then(|| value.clone())
                        .flatten(),
                    child_label: None,
                    placeholder: get(16),
                    description: get(2),
                },
                role,
                subrole,
                value,
                states: NodeAttrStates {
                    enabled: parse_enabled(get(4)),
                    control: NodeControlStates {
                        focused: parse_bool_attr(get(5)),
                        expanded: parse_bool_attr(get(6)),
                        disclosing: parse_bool_attr(get(7)),
                        selected: parse_bool_attr(get(8)),
                        readonly: readonly.value,
                    },
                    semantic: NodeSemanticStates {
                        hidden: parse_bool_attr(get(9)),
                        busy: parse_bool_attr(get(10)),
                        modal: parse_bool_attr(get(11)),
                        required: parse_bool_attr(get(12)),
                    },
                },
                bounds: position
                    .zip(size)
                    .and_then(|(position, size)| rect_from_parts(position, size)),
                has_scrollbars,
            },
            identifiers: NodeIdentifiers::from_fields(
                identifier_field(AX_IDENTIFIER),
                identifier_field(AX_DOM_IDENTIFIER),
            ),
            metrics: crate::tree::node_attribute_metrics::NodeAttributeMetrics {
                batch_reads,
                requested_count,
                fallback_reads,
                settable_reads: u64::from(readonly.attempted),
                deadline_exhausted,
            },
            status,
        }
    }

    fn failed_node_attribute_read(
        error: i32,
        requested_count: u64,
        deadline_exhausted: bool,
        batch_reads: u64,
    ) -> NodeAttributeRead {
        let mut status = NodeAttributeStatus::default();
        status.record_batch_error(error);
        NodeAttributeRead {
            attrs: NodeAttrs::default(),
            identifiers: NodeIdentifiers::from_fields(
                agent_desktop_core::LocatorField::Unknown,
                agent_desktop_core::LocatorField::Unknown,
            ),
            metrics: crate::tree::node_attribute_metrics::NodeAttributeMetrics {
                batch_reads,
                requested_count,
                fallback_reads: 0,
                settable_reads: 0,
                deadline_exhausted,
            },
            status,
        }
    }

    fn title_ui_element_text(
        element: &AXElement,
        deadline: std::time::Instant,
        usage: &mut crate::tree::observation_usage::ObservationUsage,
    ) -> Result<Option<crate::tree::bounded_string::BoundedString>, i32> {
        crate::tree::locator_deadline::prepare(element, deadline)
            .map_err(|_| accessibility_sys::kAXErrorCannotComplete)?;
        let title = crate::tree::attributes::copy_string_attr_bounded_result(
            element,
            kAXTitleAttribute,
            deadline,
            usage,
        )?;
        if let Some(title) = title.filter(|value| !value.value.trim().is_empty()) {
            return Ok(Some(title));
        }
        let role = read_unbounded_identity(element, kAXRoleAttribute, deadline)?;
        let subrole = read_unbounded_identity(element, kAXSubroleAttribute, deadline)?;
        let safe_static_text = role.as_deref() == Some("AXStaticText")
            && subrole.as_deref() != Some("AXSecureTextField");
        if safe_static_text {
            crate::tree::locator_deadline::prepare(element, deadline)
                .map_err(|_| accessibility_sys::kAXErrorCannotComplete)?;
            if let Some(value) =
                crate::tree::attributes::copy_value_typed_bounded_result(element, deadline, usage)?
                    .filter(|value| !value.value.trim().is_empty())
            {
                return Ok(Some(value));
            }
        }
        crate::tree::locator_deadline::prepare(element, deadline)
            .map_err(|_| accessibility_sys::kAXErrorCannotComplete)?;
        crate::tree::attributes::copy_string_attr_bounded_result(
            element,
            kAXDescriptionAttribute,
            deadline,
            usage,
        )
    }

    fn read_unbounded_identity(
        element: &AXElement,
        attribute: &str,
        deadline: std::time::Instant,
    ) -> Result<Option<String>, i32> {
        crate::tree::locator_deadline::prepare(element, deadline)
            .map_err(|_| accessibility_sys::kAXErrorCannotComplete)?;
        crate::tree::attributes::copy_string_attr_result(element, attribute, deadline)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use crate::tree::{
        NodeAttrs, ax_element::AXElement, node_attribute_read::NodeAttributeRead,
        node_identifiers::NodeIdentifiers,
    };

    pub(crate) fn fetch_node_attrs_with_status_for(
        _element: &AXElement,
        _requirements: agent_desktop_core::EvidenceRequirements,
        _deadline: std::time::Instant,
        _usage: &mut crate::tree::observation_usage::ObservationUsage,
    ) -> NodeAttributeRead {
        NodeAttributeRead {
            attrs: NodeAttrs::default(),
            identifiers: NodeIdentifiers::default(),
            metrics: crate::tree::node_attribute_metrics::NodeAttributeMetrics::default(),
            status: crate::tree::node_attribute_status::NodeAttributeStatus::default(),
        }
    }
}

pub(crate) use imp::fetch_node_attrs_with_status_for;
