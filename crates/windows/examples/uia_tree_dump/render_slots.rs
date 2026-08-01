//! How one read becomes one field in the capture, and what is withheld.
//!
//! Every rule that decides what a value looks like in a committed capture
//! lives here, so widening one of them cannot be done in a place that misses
//! the others.

use agent_desktop_core::LocatorField;
use agent_desktop_windows::tree::properties::{PropertyOutcome, PropertyValue};
use serde_json::{Value, json};

/// Substituted for every run-varying host value, matching the placeholders the
/// 2.0 captures already use.
const REDACTED_PID: &str = "<pid>";
const REDACTED_PROVIDER: &str = "<providerid>";
const REDACTED_PATH: &str = "<userprofile>";

/// Reports an unresolvable target as a structured skip.
///
/// Never a silent empty dump: a capture containing nothing reads as evidence
/// that the tree was empty.
pub fn skipped(reason: &str) -> String {
    json!({ "status": "skipped", "reason": reason }).to_string()
}

/// Replaces every run-varying or host-identifying value in a provider string.
///
/// `ProviderDescription` embeds a process id and a hexadecimal provider id,
/// and a user path can appear in a module path, so the substitution runs
/// before anything reaches the capture file. Each key consumes its **whole**
/// value: a substitution that stops at the first non-digit leaves the tail of
/// a `0x`-prefixed id in the file, which is a leak that reads as redacted.
pub fn normalise(text: &str) -> String {
    let substituted = substitute_after(text, "pid:", REDACTED_PID);
    let substituted = substitute_after(&substituted, "providerId:", REDACTED_PROVIDER);
    redact_paths(&substituted)
}

fn substitute_after(text: &str, key: &str, replacement: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(key) {
        out.push_str(&rest[..at + key.len()]);
        let value = &rest[at + key.len()..];
        let consumed = value
            .find(|character: char| !(character.is_ascii_alphanumeric()))
            .unwrap_or(value.len());
        if consumed == 0 {
            rest = value;
            continue;
        }
        out.push_str(replacement);
        rest = &value[consumed..];
    }
    out.push_str(rest);
    out
}

fn redact_paths(text: &str) -> String {
    let lowered = text.to_ascii_lowercase();
    match lowered.find(r"\users\") {
        None => text.to_string(),
        Some(at) => {
            let tail = &text[at + r"\users\".len()..];
            let end = tail.find('\\').unwrap_or(tail.len());
            format!("{}{}{}", &text[..at], REDACTED_PATH, &tail[end..])
        }
    }
}

pub fn text_of(outcome: &PropertyOutcome) -> Option<String> {
    match outcome {
        PropertyOutcome::Known(PropertyValue::Text(value)) => Some(value.clone()),
        _ => None,
    }
}

pub fn slot(outcome: &PropertyOutcome) -> Value {
    match outcome {
        PropertyOutcome::Known(PropertyValue::Text(value)) => json!(normalise(value)),
        PropertyOutcome::Known(PropertyValue::Flag(value)) => json!(value),
        PropertyOutcome::Known(PropertyValue::Number(value)) => json!(value),
        PropertyOutcome::Known(PropertyValue::Bounds(bounds)) => json!({
            "x": bounds.x, "y": bounds.y, "width": bounds.width, "height": bounds.height
        }),
        PropertyOutcome::Absent => json!("<absent>"),
        PropertyOutcome::Unknown => json!("<unknown>"),
    }
}

/// Records a text slot as presence and length only, never its content.
///
/// **Every value-bearing property gets this treatment, not only `Name`.** The
/// rule previously named `Name` alone, and `slot()` renders any other
/// `Known(Text(..))` verbatim - so `HelpText`, `FullDescription` and any
/// `LegacyIAccessible` string would have landed in a committed capture as
/// literal text read out of somebody's real application.
pub fn text_presence(outcome: &PropertyOutcome) -> Value {
    match text_of(outcome) {
        Some(value) => json!({ "present": true, "chars": value.chars().count() }),
        None => json!({ "present": false, "outcome": slot(outcome) }),
    }
}

/// The evidence field's own presence, taken from the resolved slot rather than
/// from a raw property, so it reports what a consumer would actually receive.
pub fn field_presence(field: &LocatorField<String>) -> Value {
    match field {
        LocatorField::Known(value) => json!({ "present": true, "chars": value.chars().count() }),
        LocatorField::Absent => json!({ "present": false, "outcome": "<absent>" }),
        LocatorField::Unknown => json!({ "present": false, "outcome": "<unknown>" }),
    }
}

pub fn field_list(field: &LocatorField<Vec<String>>) -> Value {
    match field {
        LocatorField::Known(values) => json!(values),
        LocatorField::Absent => json!("<absent>"),
        LocatorField::Unknown => json!("<unknown>"),
    }
}

pub fn role_of(field: &LocatorField<String>) -> Value {
    match field {
        LocatorField::Known(role) => json!(role),
        LocatorField::Absent => json!("<absent>"),
        LocatorField::Unknown => json!("<unknown>"),
    }
}
