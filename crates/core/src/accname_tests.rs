use super::*;

fn evidence() -> NameEvidence {
    NameEvidence {
        explicit_label: Some("explicit".into()),
        labelled_by_text: Some("labelled".into()),
        native_title: Some("title".into()),
        static_value: Some("value".into()),
        child_label: Some("child".into()),
        placeholder: Some("placeholder".into()),
        description: Some("description".into()),
    }
}

fn uncertain(slot: SlotStatus) -> NameSlotStatus {
    NameSlotStatus {
        native_title: slot,
        ..NameSlotStatus::default()
    }
}

/// The order asserted here is the one that has produced every snapshot this
/// product has ever emitted. It was not the order this module documented:
/// `description` ranked last here and fifth in the adapter that actually ran,
/// and the documented version had no caller to notice. The shipped order won,
/// and this test is what pins the two together.
#[test]
fn name_precedence_is_the_shipped_order_and_is_complete() {
    let mut value = evidence();
    assert_eq!(compute_name(&value).as_deref(), Some("explicit"));
    value.explicit_label = None;
    assert_eq!(compute_name(&value).as_deref(), Some("labelled"));
    value.labelled_by_text = None;
    assert_eq!(compute_name(&value).as_deref(), Some("title"));
    value.native_title = None;
    assert_eq!(compute_name(&value).as_deref(), Some("value"));
    value.static_value = None;
    assert_eq!(compute_name(&value).as_deref(), Some("description"));
    value.description = None;
    assert_eq!(compute_name(&value).as_deref(), Some("child"));
    value.child_label = None;
    assert_eq!(compute_name(&value).as_deref(), Some("placeholder"));
}

/// The two entry points cannot drift, because there is one array of sources
/// and both read it. This walks every prefix of the precedence and asserts
/// they agree on each.
#[test]
fn the_uncertainty_aware_and_plain_entry_points_agree_on_every_input() {
    let mut value = evidence();
    let slots: [fn(&mut NameEvidence); 7] = [
        |evidence| evidence.explicit_label = None,
        |evidence| evidence.labelled_by_text = None,
        |evidence| evidence.native_title = None,
        |evidence| evidence.static_value = None,
        |evidence| evidence.description = None,
        |evidence| evidence.child_label = None,
        |evidence| evidence.placeholder = None,
    ];
    for clear in slots {
        assert_eq!(
            resolve_name(&value, &NameSlotStatus::default())
                .known()
                .cloned(),
            compute_name(&value)
        );
        assert_eq!(
            resolve_description(&value, &NameSlotStatus::default())
                .known()
                .cloned(),
            compute_description(&value)
        );
        clear(&mut value);
    }
}

/// The behaviour an `Option`-returning precedence structurally cannot express.
/// A weaker source has a value, but the source that would have outranked it
/// failed to read - so the honest answer is that the name is not known, not
/// that it is the weaker one.
#[test]
fn a_failed_stronger_source_clouds_a_weaker_one_that_did_read() {
    let value = NameEvidence {
        child_label: Some("child".into()),
        ..NameEvidence::default()
    };

    assert_eq!(
        resolve_name(&value, &NameSlotStatus::default()),
        LocatorField::Known("child".into())
    );
    assert_eq!(
        resolve_name(&value, &uncertain(SlotStatus::Uncertain)),
        LocatorField::Unknown
    );
}

/// A suppressed slot is not a failed one: the platform is saying the slot does
/// not apply to this element, which contributes neither a value nor doubt.
/// Conflating the two would make every role that does not use a slot lose its
/// name whenever that slot's read happened to fail.
#[test]
fn a_suppressed_slot_contributes_neither_a_value_nor_uncertainty() {
    let value = NameEvidence {
        native_title: Some("Save".into()),
        child_label: Some("child".into()),
        ..NameEvidence::default()
    };

    assert_eq!(
        resolve_name(&value, &uncertain(SlotStatus::Suppressed)),
        LocatorField::Known("child".into()),
        "the suppressed slot is skipped entirely, value and all"
    );
}

#[test]
fn nothing_readable_and_nothing_failed_is_absent_rather_than_unknown() {
    assert_eq!(
        resolve_name(&NameEvidence::default(), &NameSlotStatus::default()),
        LocatorField::Absent
    );
    assert_eq!(
        resolve_name(&NameEvidence::default(), &uncertain(SlotStatus::Uncertain)),
        LocatorField::Unknown
    );
}

#[test]
fn blank_evidence_is_ignored_without_rewriting_content() {
    let value = NameEvidence {
        explicit_label: Some(" \t ".into()),
        native_title: Some("  Save  ".into()),
        ..NameEvidence::default()
    };

    assert_eq!(compute_name(&value).as_deref(), Some("  Save  "));
}

#[test]
fn description_is_not_duplicated_when_it_supplies_the_name() {
    let value = NameEvidence {
        description: Some("Only description".into()),
        ..NameEvidence::default()
    };

    assert_eq!(compute_name(&value).as_deref(), Some("Only description"));
    assert_eq!(compute_description(&value), None);
}

#[test]
fn description_remains_separate_when_a_stronger_name_exists() {
    let value = NameEvidence {
        native_title: Some("Search".into()),
        description: Some("Filters all messages".into()),
        ..NameEvidence::default()
    };

    assert_eq!(compute_name(&value).as_deref(), Some("Search"));
    assert_eq!(
        compute_description(&value).as_deref(),
        Some("Filters all messages")
    );
}

/// `placeholder` is weaker than `description` in the shipped order, so a
/// placeholder does not turn a description into a description.
#[test]
fn a_placeholder_does_not_outrank_a_description() {
    let value = NameEvidence {
        placeholder: Some("Search".into()),
        description: Some("Filters all messages".into()),
        ..NameEvidence::default()
    };

    assert_eq!(
        compute_name(&value).as_deref(),
        Some("Filters all messages")
    );
    assert_eq!(compute_description(&value), None);
}
