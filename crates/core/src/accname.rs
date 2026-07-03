use serde::{Deserialize, Serialize};

/// Raw, unreduced evidence an adapter gathers for one element's accessible
/// name/description. Fields carry native attribute text as-is; the
/// precedence between them is decided exclusively by [`compute_name`] and
/// [`compute_description`] in this module — adapters must never apply their
/// own fallback chain (see `docs/plans/.../KTD6`).
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NameEvidence {
    /// Rung 1 — an explicit, authoritative label distinct from a
    /// labelled-by reference. UIA: `Name` when sourced from an explicit
    /// `AutomationProperties.Name`-style override. AT-SPI: an explicit
    /// `label` property, when the toolkit exposes one separately from the
    /// `LABELLED_BY` relation. macOS has no attribute distinct from
    /// `AXTitleUIElement`, so this rung is always `None` on macOS today.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explicit_label: Option<String>,
    /// Rung 2 — text drawn from another element the platform designates as
    /// this element's label. macOS: `AXTitleUIElement` (resolved to that
    /// element's own title/value text). UIA: `LabeledBy`. AT-SPI:
    /// `LABELLED_BY` relation target's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labelled_by_text: Option<String>,
    /// Rung 3 — the element's own native title/name string. macOS:
    /// `AXTitle`. UIA: `Name` (default, unlabelled case). AT-SPI: `name`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_title: Option<String>,
    /// Rung 4 — the element's value promoted to a name, only for
    /// non-interactive, static/read-only roles (e.g. macOS `AXValue` on
    /// `AXStaticText`; UIA `ValuePattern.Value` on a `Text` control). The
    /// adapter is responsible for the role gating; core treats presence of
    /// this field as sufficient.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_role_value: Option<String>,
    /// Rung 5 — text aggregated from descendant labels in document order,
    /// for containers with no direct label of their own (a toolbar button
    /// wrapping an icon + text child, for example). Adapters build this
    /// with [`join_child_labels`] over the per-child text they collect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_label: Option<String>,
    /// Rung 6 — placeholder/hint text shown while the control is empty.
    /// macOS: `AXPlaceholderValue`. UIA: `HelpText` used as a placeholder
    /// fallback. AT-SPI: `placeholder-text`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    /// Rung 7 (name fallback of last resort) and also the accessible
    /// description proper via [`compute_description`]. macOS: `AXDescription`.
    /// UIA: `HelpText`. AT-SPI: `description`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Reduces [`NameEvidence`] to a single accessible name following the
/// documented 7-rung precedence: explicit label, labelled-by text, native
/// title, static-role value, aggregated child label, placeholder,
/// description last. The earliest non-empty rung wins; core never re-derives
/// evidence, it only picks among what the adapter supplied.
pub fn compute_name(evidence: &NameEvidence) -> Option<String> {
    non_empty(&evidence.explicit_label)
        .or_else(|| non_empty(&evidence.labelled_by_text))
        .or_else(|| non_empty(&evidence.native_title))
        .or_else(|| non_empty(&evidence.static_role_value))
        .or_else(|| non_empty(&evidence.child_label))
        .or_else(|| non_empty(&evidence.placeholder))
        .or_else(|| non_empty(&evidence.description))
}

/// Reduces [`NameEvidence`] to the accessible description: the raw
/// description rung, independent of whether it was also consumed as the
/// name's rung-7 fallback.
pub fn compute_description(evidence: &NameEvidence) -> Option<String> {
    non_empty(&evidence.description)
}

/// Joins per-child label text into a single aggregated child-label rung,
/// in the document order the caller supplies them, trimming and dropping
/// empty entries. Returns `None` when nothing survives.
pub fn join_child_labels<'a, I: IntoIterator<Item = &'a str>>(labels: I) -> Option<String> {
    let joined = labels
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if joined.is_empty() {
        None
    } else {
        Some(joined)
    }
}

#[cfg(test)]
#[path = "accname_tests.rs"]
mod tests;
