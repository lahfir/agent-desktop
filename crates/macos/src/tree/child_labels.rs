use crate::tree::AXElement;

pub(crate) const MAX_LABEL_ELEMENTS: usize = 5;

pub(crate) fn complete_name_evidence_with_deadline(
    attrs: &crate::tree::NodeAttrs,
    children: &[AXElement],
    deadline: std::time::Instant,
    stats: &mut agent_desktop_core::LocatorStats,
    usage: &mut crate::tree::observation_usage::ObservationUsage,
) -> Result<(agent_desktop_core::NameEvidence, bool), agent_desktop_core::AdapterError> {
    let mut evidence = attrs.name_evidence.clone();
    if !should_read_child_label(&evidence) {
        return Ok((evidence, true));
    }
    let (label, complete) = label_from_children(children, deadline, stats, usage)?;
    evidence.child_label = label;
    Ok((evidence, complete))
}

fn should_read_child_label(evidence: &agent_desktop_core::NameEvidence) -> bool {
    !has_name_without_child_content(evidence)
}

fn has_name_without_child_content(evidence: &agent_desktop_core::NameEvidence) -> bool {
    [
        evidence.explicit_label.as_deref(),
        evidence.labelled_by_text.as_deref(),
        evidence.native_title.as_deref(),
        evidence.static_value.as_deref(),
        evidence.description.as_deref(),
    ]
    .into_iter()
    .any(|value| value.is_some_and(|value| !value.trim().is_empty()))
}

#[cfg(target_os = "macos")]
fn label_from_children(
    children: &[AXElement],
    deadline: std::time::Instant,
    stats: &mut agent_desktop_core::LocatorStats,
    usage: &mut crate::tree::observation_usage::ObservationUsage,
) -> Result<(Option<String>, bool), agent_desktop_core::AdapterError> {
    let mut labels = Vec::new();
    note_label_limit(children.len(), stats);
    let mut complete = children.len() <= MAX_LABEL_ELEMENTS;
    for child in children.iter().take(MAX_LABEL_ELEMENTS) {
        let (role, role_complete) = timed_string(child, "AXRole", deadline, stats, usage)?;
        complete &= role_complete;
        match role.as_deref() {
            Some("AXStaticText") => {
                let (subrole, subrole_complete) =
                    timed_string(child, "AXSubrole", deadline, stats, usage)?;
                complete &= subrole_complete;
                if subrole.as_deref() != Some("AXSecureTextField") {
                    complete &= push_static_text(&mut labels, child, deadline, stats, usage)?;
                }
            }
            Some("AXCell") | Some("AXGroup") => {
                let (title, title_complete) =
                    timed_string(child, "AXTitle", deadline, stats, usage)?;
                complete &= title_complete;
                if let Some(title) = title {
                    labels.push(title);
                }
                let grandchildren = crate::tree::query::child_read::read_children(
                    child,
                    role.as_deref(),
                    MAX_LABEL_ELEMENTS,
                    deadline,
                );
                record_child_read(&grandchildren, stats)?;
                complete &= grandchildren.complete && !grandchildren.truncated();
                for grandchild in grandchildren.elements {
                    let (role, role_complete) =
                        timed_string(&grandchild, "AXRole", deadline, stats, usage)?;
                    complete &= role_complete;
                    if role.as_deref() == Some("AXStaticText") {
                        let (subrole, subrole_complete) =
                            timed_string(&grandchild, "AXSubrole", deadline, stats, usage)?;
                        complete &= subrole_complete;
                        if subrole.as_deref() != Some("AXSecureTextField") {
                            complete &=
                                push_static_text(&mut labels, &grandchild, deadline, stats, usage)?;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    let (label, join_complete) = join_unique_labels(labels, usage);
    Ok((label, complete && join_complete))
}

#[cfg(not(target_os = "macos"))]
fn label_from_children(
    _children: &[AXElement],
    _deadline: std::time::Instant,
    _stats: &mut agent_desktop_core::LocatorStats,
    _usage: &mut crate::tree::observation_usage::ObservationUsage,
) -> Result<(Option<String>, bool), agent_desktop_core::AdapterError> {
    Ok((None, true))
}

#[cfg(target_os = "macos")]
fn timed_string(
    element: &AXElement,
    attribute: &str,
    deadline: std::time::Instant,
    stats: &mut agent_desktop_core::LocatorStats,
    usage: &mut crate::tree::observation_usage::ObservationUsage,
) -> Result<(Option<String>, bool), agent_desktop_core::AdapterError> {
    crate::tree::locator_deadline::prepare(element, deadline)?;
    stats.semantic_reads.child_label_reads += 1;
    let value = crate::tree::attributes::copy_string_attr_bounded_result(
        element, attribute, deadline, usage,
    )
    .map_err(|error| crate::tree::query::read_error::semantic_read(error, "child_label.text"))?;
    Ok(complete_text(value, stats))
}

#[cfg(target_os = "macos")]
fn push_static_text(
    labels: &mut Vec<String>,
    element: &AXElement,
    deadline: std::time::Instant,
    stats: &mut agent_desktop_core::LocatorStats,
    usage: &mut crate::tree::observation_usage::ObservationUsage,
) -> Result<bool, agent_desktop_core::AdapterError> {
    crate::tree::locator_deadline::prepare(element, deadline)?;
    stats.semantic_reads.child_label_reads += 1;
    let value = crate::tree::attributes::copy_value_typed_bounded_result(element, deadline, usage)
        .map_err(|error| {
            crate::tree::query::read_error::semantic_read(error, "child_label.value")
        })?;
    let (mut text, mut complete) = complete_text(value, stats);
    if text.is_none() && complete {
        let (title, title_complete) = timed_string(element, "AXTitle", deadline, stats, usage)?;
        text = title;
        complete &= title_complete;
    }
    if let Some(text) = text {
        labels.push(text);
    }
    Ok(complete)
}

fn complete_text(
    value: Option<crate::tree::bounded_string::BoundedString>,
    stats: &mut agent_desktop_core::LocatorStats,
) -> (Option<String>, bool) {
    match value {
        Some(value) if value.complete => (Some(value.value), true),
        Some(_) => {
            stats.traversal.limits.text_hits += 1;
            (None, false)
        }
        None => (None, true),
    }
}

#[cfg(target_os = "macos")]
fn record_child_read(
    read: &crate::tree::query::child_read::ChildRead,
    stats: &mut agent_desktop_core::LocatorStats,
) -> Result<(), agent_desktop_core::AdapterError> {
    stats.reads.child_reads += read.status.attempts;
    stats.reads.cannot_complete += read.status.cannot_complete;
    stats.reads.native_read_failures += read.status.native_read_failures;
    stats.reads.deadline_exhausted += u64::from(read.status.deadline_exhausted);
    stats.traversal.limits.child_count_changes += u64::from(read.status.count_changed);
    stats.traversal.limits.child_label_hits += u64::from(read.truncated());
    stats.traversal.limits.child_hits += u64::from(read.status.cursor_stalled);
    if read.status.api_disabled {
        return Err(crate::tree::query::read_error::semantic_read(
            accessibility_sys::kAXErrorAPIDisabled,
            "child_label.children",
        ));
    }
    if read.status.invalid_element {
        return Err(crate::tree::query::read_error::semantic_read(
            accessibility_sys::kAXErrorInvalidUIElement,
            "child_label.children",
        ));
    }
    Ok(())
}

fn note_label_limit(child_count: usize, stats: &mut agent_desktop_core::LocatorStats) {
    stats.traversal.limits.child_label_hits += u64::from(child_count > MAX_LABEL_ELEMENTS);
}

fn join_unique_labels(
    labels: impl IntoIterator<Item = String>,
    usage: &mut crate::tree::observation_usage::ObservationUsage,
) -> (Option<String>, bool) {
    let mut joined = String::new();
    let mut seen = Vec::new();
    let capacity = usage.string_capacity();
    let mut complete = true;
    for label in labels {
        let normalized = label.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() || seen.iter().any(|value| value == &normalized) {
            continue;
        }
        let separator = usize::from(!joined.is_empty());
        let remaining = capacity.saturating_sub(joined.len().saturating_add(separator));
        if remaining == 0 {
            complete = false;
            break;
        }
        if separator == 1 {
            joined.push(' ');
        }
        let mut end = remaining.min(normalized.len());
        while !normalized.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        joined.push_str(&normalized[..end]);
        complete &= end == normalized.len();
        seen.push(normalized);
        if !complete {
            break;
        }
    }
    usage.claim_text(joined.len());
    ((!joined.is_empty()).then_some(joined), complete)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_desktop_core::ObservationBudget;

    fn usage(max_field_bytes: usize) -> crate::tree::observation_usage::ObservationUsage {
        crate::tree::observation_usage::ObservationUsage::new(ObservationBudget {
            max_field_bytes,
            max_text_bytes: max_field_bytes,
            ..ObservationBudget::default()
        })
    }

    #[test]
    fn child_label_cap_is_reported_as_incomplete_traversal() {
        let mut stats = agent_desktop_core::LocatorStats::default();

        note_label_limit(MAX_LABEL_ELEMENTS + 1, &mut stats);

        assert_eq!(stats.traversal.limits.child_label_hits, 1);
    }

    #[test]
    fn labels_are_normalized_deduplicated_and_joined_in_document_order() {
        let mut usage = usage(256);
        let labels = vec![
            "  Save\n".to_string(),
            "Save".to_string(),
            " Draft   title ".to_string(),
        ];

        assert_eq!(
            join_unique_labels(labels, &mut usage),
            (Some("Save Draft title".into()), true)
        );
    }

    #[test]
    fn label_budget_truncation_is_explicit_and_utf8_safe() {
        let mut usage = usage(4);

        let (label, complete) = join_unique_labels(["a🙂z".into()], &mut usage);

        assert_eq!(label.as_deref(), Some("a"));
        assert!(!complete);
    }

    #[test]
    fn description_only_name_skips_child_content_fallback() {
        let evidence = agent_desktop_core::NameEvidence {
            description: Some("scroll-area".into()),
            ..Default::default()
        };

        assert!(!should_read_child_label(&evidence));
    }

    #[test]
    fn unnamed_elements_still_use_bounded_child_content_fallback() {
        assert!(should_read_child_label(
            &agent_desktop_core::NameEvidence::default()
        ));
    }

    #[test]
    fn direct_names_skip_child_label_reads_for_every_role() {
        let evidence = agent_desktop_core::NameEvidence {
            explicit_label: Some("scroll-area".into()),
            ..Default::default()
        };

        assert!(!should_read_child_label(&evidence));
    }
}
