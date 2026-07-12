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

#[test]
fn name_precedence_is_platform_neutral_and_complete() {
    let mut value = evidence();
    assert_eq!(compute_name(&value).as_deref(), Some("explicit"));
    value.explicit_label = None;
    assert_eq!(compute_name(&value).as_deref(), Some("labelled"));
    value.labelled_by_text = None;
    assert_eq!(compute_name(&value).as_deref(), Some("title"));
    value.native_title = None;
    assert_eq!(compute_name(&value).as_deref(), Some("value"));
    value.static_value = None;
    assert_eq!(compute_name(&value).as_deref(), Some("child"));
    value.child_label = None;
    assert_eq!(compute_name(&value).as_deref(), Some("placeholder"));
    value.placeholder = None;
    assert_eq!(compute_name(&value).as_deref(), Some("description"));
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
fn description_remains_separate_when_an_earlier_name_exists() {
    let value = NameEvidence {
        placeholder: Some("Search".into()),
        description: Some("Filters all messages".into()),
        ..NameEvidence::default()
    };

    assert_eq!(compute_name(&value).as_deref(), Some("Search"));
    assert_eq!(
        compute_description(&value).as_deref(),
        Some("Filters all messages")
    );
}
