use crate::name_evidence::NameEvidence;

pub fn compute_name(evidence: &NameEvidence) -> Option<String> {
    [
        evidence.explicit_label.as_deref(),
        evidence.labelled_by_text.as_deref(),
        evidence.native_title.as_deref(),
        evidence.static_value.as_deref(),
        evidence.child_label.as_deref(),
        evidence.placeholder.as_deref(),
        evidence.description.as_deref(),
    ]
    .into_iter()
    .find_map(non_blank)
    .map(str::to_string)
}

pub fn compute_description(evidence: &NameEvidence) -> Option<String> {
    let description = non_blank(evidence.description.as_deref())?;
    [
        evidence.explicit_label.as_deref(),
        evidence.labelled_by_text.as_deref(),
        evidence.native_title.as_deref(),
        evidence.static_value.as_deref(),
        evidence.child_label.as_deref(),
        evidence.placeholder.as_deref(),
    ]
    .into_iter()
    .any(|candidate| non_blank(candidate).is_some())
    .then(|| description.to_string())
}

fn non_blank(value: Option<&str>) -> Option<&str> {
    value.filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
#[path = "accname_tests.rs"]
mod tests;
