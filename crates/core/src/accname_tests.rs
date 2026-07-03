use super::*;

fn evidence_all_rungs() -> NameEvidence {
    NameEvidence {
        explicit_label: Some("explicit".into()),
        labelled_by_text: Some("labelled-by".into()),
        native_title: Some("native-title".into()),
        static_role_value: Some("static-value".into()),
        child_label: Some("child-label".into()),
        placeholder: Some("placeholder".into()),
        description: Some("description".into()),
    }
}

#[test]
fn rung1_explicit_label_wins_over_every_lower_rung() {
    let evidence = evidence_all_rungs();
    assert_eq!(compute_name(&evidence).as_deref(), Some("explicit"));
}

#[test]
fn rung2_labelled_by_text_wins_when_explicit_absent() {
    let mut evidence = evidence_all_rungs();
    evidence.explicit_label = None;
    assert_eq!(compute_name(&evidence).as_deref(), Some("labelled-by"));
}

#[test]
fn rung3_native_title_wins_when_rungs_1_2_absent() {
    let mut evidence = evidence_all_rungs();
    evidence.explicit_label = None;
    evidence.labelled_by_text = None;
    assert_eq!(compute_name(&evidence).as_deref(), Some("native-title"));
}

#[test]
fn rung4_static_role_value_wins_when_rungs_1_3_absent() {
    let mut evidence = evidence_all_rungs();
    evidence.explicit_label = None;
    evidence.labelled_by_text = None;
    evidence.native_title = None;
    assert_eq!(compute_name(&evidence).as_deref(), Some("static-value"));
}

#[test]
fn rung5_child_label_wins_when_rungs_1_4_absent() {
    let mut evidence = evidence_all_rungs();
    evidence.explicit_label = None;
    evidence.labelled_by_text = None;
    evidence.native_title = None;
    evidence.static_role_value = None;
    assert_eq!(compute_name(&evidence).as_deref(), Some("child-label"));
}

#[test]
fn rung6_placeholder_wins_when_rungs_1_5_absent() {
    let mut evidence = evidence_all_rungs();
    evidence.explicit_label = None;
    evidence.labelled_by_text = None;
    evidence.native_title = None;
    evidence.static_role_value = None;
    evidence.child_label = None;
    assert_eq!(compute_name(&evidence).as_deref(), Some("placeholder"));
}

#[test]
fn rung7_description_is_last_resort_for_name() {
    let mut evidence = evidence_all_rungs();
    evidence.explicit_label = None;
    evidence.labelled_by_text = None;
    evidence.native_title = None;
    evidence.static_role_value = None;
    evidence.child_label = None;
    evidence.placeholder = None;
    assert_eq!(compute_name(&evidence).as_deref(), Some("description"));
}

#[test]
fn all_absent_evidence_computes_no_name() {
    let evidence = NameEvidence::default();
    assert_eq!(compute_name(&evidence), None);
}

#[test]
fn all_blank_evidence_computes_no_name() {
    let evidence = NameEvidence {
        explicit_label: Some("   ".into()),
        labelled_by_text: Some("".into()),
        native_title: Some("\t".into()),
        static_role_value: Some(String::new()),
        child_label: Some("  ".into()),
        placeholder: Some(String::new()),
        description: Some(" ".into()),
    };
    assert_eq!(compute_name(&evidence), None);
}

#[test]
fn compute_description_returns_description_rung_independent_of_name() {
    let evidence = evidence_all_rungs();
    assert_eq!(
        compute_description(&evidence).as_deref(),
        Some("description")
    );
    assert_eq!(compute_name(&evidence).as_deref(), Some("explicit"));
}

#[test]
fn compute_description_none_when_description_absent() {
    let mut evidence = evidence_all_rungs();
    evidence.description = None;
    assert_eq!(compute_description(&evidence), None);
}

#[test]
fn child_label_aggregation_joins_in_document_order() {
    let joined = join_child_labels(["Save", "As...", "PDF"]);
    assert_eq!(joined.as_deref(), Some("Save As... PDF"));
}

#[test]
fn child_label_aggregation_drops_blank_entries_but_preserves_order() {
    let joined = join_child_labels(["", "First", "   ", "Second"]);
    assert_eq!(joined.as_deref(), Some("First Second"));
}

#[test]
fn child_label_aggregation_of_all_blank_entries_is_none() {
    assert_eq!(join_child_labels(["", "   ", "\t"]), None);
}

#[test]
fn child_label_aggregation_of_empty_iterator_is_none() {
    assert_eq!(join_child_labels(std::iter::empty()), None);
}

#[test]
fn name_evidence_serde_roundtrip_skips_absent_fields() {
    let evidence = NameEvidence {
        native_title: Some("Only Title".into()),
        ..Default::default()
    };
    let json = serde_json::to_value(&evidence).expect("serialize");
    assert_eq!(json, serde_json::json!({ "native_title": "Only Title" }));
    let round_tripped: NameEvidence = serde_json::from_value(json).expect("deserialize");
    assert_eq!(round_tripped, evidence);
}
