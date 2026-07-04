use serde::{Deserialize, Serialize};

/// Raw accessible-name evidence an adapter gathers for one element: its own
/// title, its description, and — for static-text roles only — its value
/// promoted to a name. Each platform's `resolve_element_name` reduces this to a
/// single accessible name; the macOS precedence is title → description →
/// static value. Keeping the evidence typed (rather than returning a bare
/// `String`) keeps every name consumer — the snapshot builder, strict ref
/// re-resolution, hit-test occluder naming, and ambiguity classification —
/// reducing the *same* evidence the same way, so a stored ref name always
/// matches what the resolver recomputes.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameEvidence {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_role_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}
