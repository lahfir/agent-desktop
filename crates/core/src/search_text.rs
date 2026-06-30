pub(crate) fn normalize(value: &str) -> String {
    if value.is_ascii() {
        value.to_ascii_lowercase()
    } else {
        value.to_lowercase()
    }
}

pub(crate) fn contains(haystack: &str, normalized_needle: &str) -> bool {
    if normalized_needle.is_empty() {
        return true;
    }
    if haystack.is_ascii() && normalized_needle.is_ascii() {
        return haystack
            .as_bytes()
            .windows(normalized_needle.len())
            .any(|chunk| chunk.eq_ignore_ascii_case(normalized_needle.as_bytes()));
    }
    normalize(haystack).contains(normalized_needle)
}

pub(crate) fn node_contains(
    node: &crate::node::AccessibilityNode,
    normalized_needle: &str,
) -> bool {
    [
        node.name.as_deref(),
        node.value.as_deref(),
        node.description.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|text| contains(text, normalized_needle))
}

#[cfg(test)]
#[path = "search_text_tests.rs"]
mod tests;
