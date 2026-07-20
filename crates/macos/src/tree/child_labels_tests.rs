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

    assert!(!should_read_child_label("button", &evidence));
}

#[test]
fn unnamed_elements_still_use_bounded_child_content_fallback() {
    assert!(should_read_child_label(
        "button",
        &agent_desktop_core::NameEvidence::default()
    ));
}

#[test]
fn container_roles_never_derive_names_from_children() {
    for role in [
        "scrollarea",
        "group",
        "window",
        "list",
        "table",
        "outline",
        "toolbar",
    ] {
        assert!(
            !should_read_child_label(role, &agent_desktop_core::NameEvidence::default()),
            "container role {role} must not name itself from descendants"
        );
    }
}

#[test]
fn direct_names_skip_child_label_reads_for_every_role() {
    let evidence = agent_desktop_core::NameEvidence {
        explicit_label: Some("scroll-area".into()),
        ..Default::default()
    };

    assert!(!should_read_child_label("button", &evidence));
}

#[cfg(target_os = "macos")]
#[test]
fn transient_child_label_error_degrades_name_to_unknown_and_incomplete() {
    let mut stats = agent_desktop_core::LocatorStats::default();
    let mut usage = usage(256);
    let children = [AXElement(std::ptr::null_mut())];
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(250);

    let (evidence, complete) = complete_name_evidence_with_deadline(
        &crate::tree::NodeAttrs::default(),
        "button",
        &children,
        deadline,
        NameEvidenceSinks {
            stats: &mut stats,
            usage: &mut usage,
        },
    )
    .expect("a transient child-label read error must not abort the traversal");

    assert_eq!(evidence.child_label, None);
    assert!(!complete);
}

#[cfg(target_os = "macos")]
#[test]
fn api_disabled_child_label_reads_still_fail_closed() {
    let mut stats = agent_desktop_core::LocatorStats::default();

    let error = degrade_transient_read(
        accessibility_sys::kAXErrorAPIDisabled,
        "child_label.text",
        &mut stats,
    )
    .expect_err("API disablement must abort the observation");

    assert_eq!(error.code, agent_desktop_core::ErrorCode::PermDenied);
}

#[cfg(target_os = "macos")]
#[test]
fn transient_child_label_errors_are_recorded_in_read_health() {
    let mut stats = agent_desktop_core::LocatorStats::default();

    for error in [
        accessibility_sys::kAXErrorInvalidUIElement,
        accessibility_sys::kAXErrorCannotComplete,
        accessibility_sys::kAXErrorFailure,
    ] {
        assert_eq!(
            degrade_transient_read(error, "child_label.text", &mut stats).unwrap(),
            (None, false)
        );
    }

    assert_eq!(stats.reads.health.cannot_complete, 1);
    assert_eq!(stats.reads.health.native_read_failures, 1);
}

#[cfg(target_os = "macos")]
#[test]
fn invalid_child_list_degrades_instead_of_aborting() {
    let mut stats = agent_desktop_core::LocatorStats::default();
    let mut read = crate::tree::query::child_read::ChildRead::empty(true);
    read.status.invalid_element = true;

    assert!(record_child_read(&read, &mut stats).is_ok());
}
