use crate::{adapter::SnapshotSurface, refs::RefEntry, roles::is_mutable_value_role};

/// Returns true when a saved ref has stable text identity beyond role/path/bounds.
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

/// Compares saved ref identity against live text without treating mutable
/// control values as stable identity.
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

/// Allows a platform adapter to search replacement windows only when the saved
/// ref has enough non-text evidence for the shared classifier to fail closed.
/// A saved source-window title disables this fallback unless a platform first
/// finds that title uniquely; otherwise the old titled window is considered gone.
pub fn bounded_window_fallback_allowed(entry: &RefEntry) -> bool {
    matches!(entry.source_surface, SnapshotSurface::Window)
        && entry.source_window_id.is_some()
        && entry.source_window_title.is_none()
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
    if is_mutable_value_role(role) && value_matches_name(meaningful_text(value), name) {
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

fn value_matches_name(value: Option<&str>, name: &str) -> bool {
    value == Some(name)
        || numeric_text(value)
            .zip(numeric_text(Some(name)))
            .is_some_and(|(value, name)| value == name)
}

fn numeric_text(value: Option<&str>) -> Option<f64> {
    value
        .and_then(|text| text.parse::<f64>().ok())
        .filter(|number| number.is_finite())
}

#[cfg(test)]
#[path = "ref_identity_tests.rs"]
mod tests;
