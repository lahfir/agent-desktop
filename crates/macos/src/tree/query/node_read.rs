use agent_desktop_core::{
    AdapterError, ErrorCode, LocatorEvidence, LocatorField, LocatorRefEvidence, LocatorStats,
};
use serde_json::json;

use crate::tree::query::evidence_fields::{description_field, name_field};
use crate::tree::query::node_evidence::{
    identifiers as identifier_evidence, is_transparent_wrapper, is_wrapper_candidate, option_field,
    required_complete, unknown as unknown_evidence, update_identifier_stats,
};
use crate::tree::query::node_read_context::NodeReadContext;
use crate::tree::{AXElement, query::child_read::ChildRead};

pub(crate) struct NodeRead {
    pub(crate) attrs: crate::tree::NodeAttrs,
    pub(crate) evidence: LocatorEvidence,
    pub(crate) web_wrapper: bool,
    pub(crate) invalid_element: bool,
    pub(crate) child_read: ChildRead,
    pub(crate) evidence_complete: bool,
}

pub(crate) fn read_node(
    element: &AXElement,
    context: NodeReadContext<'_>,
) -> Result<NodeRead, AdapterError> {
    let NodeReadContext {
        tree,
        stats,
        usage,
        requirements,
        deadline,
        child_plan,
    } = context;
    crate::tree::locator_deadline::prepare(element, deadline)?;
    let read = crate::tree::element::fetch_node_attrs_with_status_for(
        element,
        requirements,
        deadline,
        usage,
    );
    record_attribute_read(
        stats,
        read.metrics.batch_reads,
        read.metrics.requested_count,
        read.metrics.fallback_reads,
        read.status.cannot_complete,
        read.status.native_read_failures,
    );
    stats.semantic_reads.settable_reads += read.metrics.settable_reads;
    stats.reads.deadline_exhausted += u64::from(read.metrics.deadline_exhausted);
    stats.traversal.limits.text_hits += read.status.text_truncations;
    if read.status.api_disabled {
        return Err(permission_error("attributes"));
    }
    let identifiers = identifier_evidence(&read.identifiers, requirements.identifiers);
    update_identifier_stats(&identifiers, stats);
    if read.status.invalid_element {
        return Ok(NodeRead {
            attrs: crate::tree::NodeAttrs::default(),
            evidence: unknown_evidence(),
            web_wrapper: false,
            invalid_element: true,
            child_read: ChildRead::empty(false),
            evidence_complete: false,
        });
    }
    let attrs = read.attrs;
    let wrapper_candidate = is_wrapper_candidate(attrs.role.as_deref(), attrs.subrole.as_deref());
    let child_read = crate::tree::query::child_read::read_children(
        element,
        attrs.role.as_deref(),
        child_plan.max_elements(wrapper_candidate),
        deadline,
    );
    stats.reads.child_reads += child_read.status.attempts;
    stats.reads.cannot_complete += child_read.status.cannot_complete;
    stats.reads.native_read_failures += child_read.status.native_read_failures;
    stats.reads.deadline_exhausted += u64::from(child_read.status.deadline_exhausted);
    stats.traversal.limits.child_count_changes += u64::from(child_read.status.count_changed);
    stats.traversal.limits.child_hits += u64::from(child_read.status.cursor_stalled);
    if child_read.status.api_disabled {
        return Err(permission_error("children"));
    }
    if child_read.status.invalid_element {
        return Ok(NodeRead {
            attrs: crate::tree::NodeAttrs::default(),
            evidence: unknown_evidence(),
            web_wrapper: false,
            invalid_element: true,
            child_read: ChildRead::empty(false),
            evidence_complete: false,
        });
    }

    let role = attrs
        .role
        .as_deref()
        .map(|role| crate::tree::roles::ax_role_and_subrole_to_str(role, attrs.subrole.as_deref()))
        .unwrap_or("unknown")
        .to_string();
    let secure = attrs.role.as_deref() == Some("AXSecureTextField")
        || attrs.subrole.as_deref() == Some("AXSecureTextField");
    let value = (!secure).then(|| attrs.value.clone()).flatten();
    let (name_evidence, child_label_complete) = if requirements.name || requirements.description {
        crate::tree::child_labels::complete_name_evidence_with_deadline(
            &attrs,
            &role,
            &child_read.elements,
            deadline,
            stats,
            usage,
        )?
    } else {
        (attrs.name_evidence.clone(), true)
    };
    let children_complete = child_read.complete && !child_read.truncated() && child_label_complete;
    let name_field = if !requirements.name {
        LocatorField::Unknown
    } else {
        name_field(
            &name_evidence,
            &read.status,
            attrs.role.as_deref(),
            children_complete,
        )
    };
    let description_field = if !requirements.description {
        LocatorField::Unknown
    } else {
        description_field(
            &name_evidence,
            &read.status,
            attrs.role.as_deref(),
            children_complete,
        )
    };
    let state_context = crate::tree::state_reader::StateReaderContext {
        focused: tree.focused.as_ref(),
        window_bounds: tree.window_bounds,
        is_secure_text: secure,
    };
    let states = requirements.states.then(|| {
        crate::tree::state_reader::states_from_element(element, &attrs, &role, &state_context)
    });
    let actions = if let Some(actions) =
        read_native_actions_if(requirements.ref_evidence.actions, || {
            crate::tree::action_list::read_platform_available_actions(
                element,
                &role,
                attrs.has_scrollbars,
                deadline,
                usage,
            )
        }) {
        stats.reads.action_reads += 1;
        stats.reads.cannot_complete += u64::from(actions.cannot_complete);
        stats.reads.deadline_exhausted += u64::from(actions.deadline_exhausted);
        stats.semantic_reads.settable_reads += actions.settable_reads;
        if actions.api_disabled {
            return Err(permission_error("actions"));
        }
        if actions.invalid_element {
            return Ok(NodeRead {
                attrs: crate::tree::NodeAttrs::default(),
                evidence: unknown_evidence(),
                web_wrapper: false,
                invalid_element: true,
                child_read: ChildRead::empty(false),
                evidence_complete: false,
            });
        }
        if actions.complete && !actions.deadline_exhausted && !read.status.scrollbars_unknown() {
            LocatorField::Known(actions.actions)
        } else {
            LocatorField::Unknown
        }
    } else {
        LocatorField::Unknown
    };
    let web_wrapper = is_transparent_wrapper(
        attrs.role.as_deref(),
        attrs.subrole.as_deref(),
        &name_evidence,
        value.as_deref(),
        &identifiers,
        &actions,
    );
    let role_field = if attrs.role.is_some() {
        LocatorField::Known(role)
    } else {
        option_field(None, read.status.role_unknown())
    };
    let evidence = LocatorEvidence {
        role: role_field,
        name: name_field,
        description: description_field,
        value: if requirements.value {
            option_field(value, read.status.value_unknown())
        } else {
            LocatorField::Unknown
        },
        identifiers,
        states: if !requirements.states || read.status.states_unknown() {
            LocatorField::Unknown
        } else {
            LocatorField::Known(states.unwrap_or_default())
        },
        ref_evidence: LocatorRefEvidence {
            bounds: if requirements.ref_evidence.bounds {
                option_field(tree.bounds_for(attrs.bounds), read.status.bounds_unknown())
            } else {
                LocatorField::Unknown
            },
            available_actions: actions,
        },
    };
    let evidence_complete = required_complete(&evidence, requirements)
        && !read.metrics.deadline_exhausted
        && !child_read.status.deadline_exhausted;
    Ok(NodeRead {
        attrs,
        evidence,
        web_wrapper,
        invalid_element: false,
        child_read,
        evidence_complete,
    })
}

fn read_native_actions_if<T>(include_actions: bool, reader: impl FnOnce() -> T) -> Option<T> {
    include_actions.then(reader)
}

fn permission_error(phase: &str) -> AdapterError {
    AdapterError::new(
        ErrorCode::PermDenied,
        "Accessibility API is disabled while reading live locator evidence",
    )
    .with_suggestion("Grant Accessibility permission, then retry")
    .with_details(json!({ "kind": "locator_api_disabled", "phase": phase }))
}

fn record_attribute_read(
    stats: &mut LocatorStats,
    batch_reads: u64,
    requested_count: u64,
    fallback_reads: u64,
    cannot_complete: bool,
    native_read_failures: u64,
) {
    stats.reads.attribute_batches += batch_reads;
    stats.reads.attributes_requested += requested_count;
    stats.reads.fallback_reads += fallback_reads;
    stats.reads.cannot_complete += u64::from(cannot_complete);
    stats.reads.native_read_failures += native_read_failures;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_stats_record_actual_batch_and_field_counts() {
        let mut stats = LocatorStats::default();

        record_attribute_read(&mut stats, 1, 22, 0, false, 0);

        assert_eq!(stats.reads.attribute_batches, 1);
        assert_eq!(stats.reads.attributes_requested, 22);
        assert_eq!(stats.reads.fallback_reads, 0);
    }

    #[test]
    fn cannot_complete_keeps_missing_fields_unknown() {
        let mut stats = LocatorStats::default();

        record_attribute_read(&mut stats, 1, 22, 22, true, 0);

        assert_eq!(stats.reads.cannot_complete, 1);
        assert_eq!(stats.reads.fallback_reads, 22);
        assert_eq!(option_field::<String>(None, true), LocatorField::Unknown);
        assert_eq!(
            option_field(Some("Save".to_string()), true),
            LocatorField::Known("Save".into())
        );
    }

    #[test]
    fn omitted_ref_evidence_performs_zero_native_action_reads() {
        let calls = std::cell::Cell::new(0);

        let read = read_native_actions_if(false, || calls.set(calls.get() + 1));

        assert!(read.is_none());
        assert_eq!(calls.get(), 0);
    }
}
