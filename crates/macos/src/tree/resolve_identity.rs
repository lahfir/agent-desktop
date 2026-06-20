use agent_desktop_core::{refs::RefEntry, roles::is_mutable_value_role};

pub(super) fn has_meaningful_identity(entry: &RefEntry) -> bool {
    meaningful_text(entry.name.as_deref()).is_some()
        || stable_value(entry.role.as_str(), entry.value.as_deref()).is_some()
        || meaningful_text(entry.description.as_deref()).is_some()
}

pub(super) fn identity_matches(
    entry: &RefEntry,
    actual_name: Option<&str>,
    actual_value: Option<&str>,
    actual_description: Option<&str>,
) -> bool {
    let expected_name = meaningful_text(entry.name.as_deref());
    let expected_value = stable_value(entry.role.as_str(), entry.value.as_deref());
    let expected_description = meaningful_text(entry.description.as_deref());
    let actual_name = meaningful_text(actual_name);
    let actual_value = stable_value(entry.role.as_str(), actual_value);
    let actual_description = meaningful_text(actual_description);

    if let Some(expected) = expected_name {
        return match_primary_identity(expected, actual_name, actual_value);
    }
    if let Some(expected) = expected_value {
        return match_primary_identity(expected, actual_value, actual_name);
    }
    if let Some(expected) = expected_description {
        return match_primary_identity(expected, actual_description, actual_name);
    }

    actual_name.is_none() && actual_value.is_none() && actual_description.is_none()
}

fn match_primary_identity(
    expected: &str,
    actual_primary: Option<&str>,
    actual_fallback: Option<&str>,
) -> bool {
    match actual_primary {
        Some(actual) => actual == expected,
        None => actual_fallback == Some(expected),
    }
}

fn meaningful_text(value: Option<&str>) -> Option<&str> {
    value.filter(|text| !text.is_empty())
}

fn stable_value<'a>(role: &str, value: Option<&'a str>) -> Option<&'a str> {
    (!is_mutable_value_role(role))
        .then(|| meaningful_text(value))
        .flatten()
}

#[cfg(test)]
#[path = "resolve_identity_tests.rs"]
mod tests;
