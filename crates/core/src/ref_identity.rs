use crate::{adapter::SnapshotSurface, refs::RefEntry, roles::is_mutable_value_role};

pub fn has_meaningful_identity(entry: &RefEntry) -> bool {
    stable_name(
        entry.role.as_str(),
        entry.name.as_deref(),
        entry.value.as_deref(),
    )
    .is_some()
        || stable_value(entry.role.as_str(), entry.value.as_deref()).is_some()
        || meaningful_text(entry.description.as_deref()).is_some()
}

pub fn identity_matches(
    entry: &RefEntry,
    actual_name: Option<&str>,
    actual_value: Option<&str>,
    actual_description: Option<&str>,
) -> bool {
    let expected_name = stable_name(
        entry.role.as_str(),
        entry.name.as_deref(),
        entry.value.as_deref(),
    );
    let expected_value = stable_value(entry.role.as_str(), entry.value.as_deref());
    let expected_description = meaningful_text(entry.description.as_deref());
    let actual_name = stable_name(entry.role.as_str(), actual_name, actual_value);
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

    if is_mutable_value_role(entry.role.as_str()) {
        return true;
    }

    actual_name.is_none() && actual_value.is_none() && actual_description.is_none()
}

pub fn bounded_window_fallback_allowed(entry: &RefEntry) -> bool {
    matches!(entry.source_surface, SnapshotSurface::Window)
        && entry.source_window_id.is_some()
        && entry.bounds_hash.is_some()
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

fn stable_name<'a>(role: &str, name: Option<&'a str>, value: Option<&str>) -> Option<&'a str> {
    let name = meaningful_text(name)?;
    if is_mutable_value_role(role) && meaningful_text(value) == Some(name) {
        None
    } else {
        Some(name)
    }
}

fn stable_value<'a>(role: &str, value: Option<&'a str>) -> Option<&'a str> {
    (!is_mutable_value_role(role))
        .then(|| meaningful_text(value))
        .flatten()
}

#[cfg(test)]
#[path = "ref_identity_tests.rs"]
mod tests;
