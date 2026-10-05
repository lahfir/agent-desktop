//! How one read becomes one field in the capture, and what is withheld.
//!
//! Every rule that decides what a value looks like in a committed capture
//! lives here, so widening one of them cannot be done in a place that misses
//! the others.

use agent_desktop_core::LocatorField;
use agent_desktop_windows::tree::properties::{PropertyOutcome, PropertyValue};
use serde_json::{Value, json};

/// Substituted for every run-varying host value, matching the placeholders the
/// earlier committed captures already use.
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

/// Replaces the user name segment of **every** profile path in the value.
///
/// A `ProviderDescription` names one entry per provider in the chain and each
/// entry carries its own module path, so a rule that stopped after the first
/// match would write the second user name into a committed capture verbatim -
/// a leak in a value that reads as redacted because the placeholder is
/// already there.
fn redact_paths(text: &str) -> String {
    const USERS: &str = r"\users\";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.to_ascii_lowercase().find(USERS) {
        out.push_str(&rest[..at]);
        out.push_str(REDACTED_PATH);
        let tail = &rest[at + USERS.len()..];
        let end = tail.find('\\').unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
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

/// Renders a resolved evidence slot, keeping the tri-state visible.
///
/// `Absent` and `Unknown` are rendered as distinct markers rather than both
/// collapsing to an empty value, because that difference is the whole point of
/// the slot: `Absent` is the provider answering, `Unknown` is a read that
/// failed. A census that showed them the same way would hide exactly the
/// distinction it exists to report.
///
/// This and [`role_of`] are the same three arms over different payloads. One
/// generic function would need a `Serialize` bound, and `serde` is not a
/// dependency of this crate - widening a shipped crate's dependency surface to
/// collapse six lines of dev tooling is the worse trade.
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

/// Regression gate for the redaction rule this module exists to enforce.
///
/// These prove `text_presence`/`field_presence` themselves withhold their
/// input. The other half of the rule - that `render_node` in `render.rs`
/// calls the withholding renderer rather than `slot()` for each text-bearing
/// property - is gated separately, by `render_node_tests.rs`: a revert there
/// of `automation_id`, `help_text`, `full_description`, `legacy_default_action`,
/// `name`, or `description` back to a verbatim renderer fails that test.
#[cfg(test)]
mod tests {
    use super::*;

    const MARKER: &str = "ZZ-CENSUS-LEAK-MARKER-do-not-serialize-1234567";

    #[test]
    fn text_presence_never_renders_the_value() {
        let outcome = PropertyOutcome::Known(PropertyValue::Text(MARKER.to_string()));

        let rendered = text_presence(&outcome);

        assert!(
            !rendered.to_string().contains(MARKER),
            "text_presence leaked its input: {rendered}"
        );
        assert_eq!(
            rendered,
            json!({ "present": true, "chars": MARKER.chars().count() })
        );
    }

    #[test]
    fn field_presence_never_renders_the_value() {
        let field = LocatorField::Known(MARKER.to_string());

        let rendered = field_presence(&field);

        assert!(
            !rendered.to_string().contains(MARKER),
            "field_presence leaked its input: {rendered}"
        );
        assert_eq!(
            rendered,
            json!({ "present": true, "chars": MARKER.chars().count() })
        );
    }

    /// A realistic `ProviderDescription`, in the shape the in-box providers
    /// publish it: a decimal process id, a hexadecimal provider id, and a
    /// module path under the running user's profile.
    const PROVIDER_DESCRIPTION: &str = concat!(
        "[pid:4812,providerId:0x1A2B3C hwnd:0x90210 ",
        "Main(parent link):Microsoft: MSAA Proxy (unmanaged:C:",
        r"\Users\devbox\app\uiautomationcore.dll)]"
    );

    /// The whole substituted value is consumed, not the leading run of digits.
    ///
    /// A substitution that stops at the first non-digit leaves the tail of a
    /// `0x`-prefixed id in the capture - `providerId:<providerid>x1A2B3C` - and
    /// that reads as redacted while still identifying the host. Asserting only
    /// that the placeholder appears is exactly the prefix-shaped check that
    /// would accept it, so the host values themselves are asserted absent.
    #[test]
    fn normalise_consumes_the_whole_value_of_every_host_identifier() {
        let normalised = normalise(PROVIDER_DESCRIPTION);

        for leaked in ["4812", "1A2B3C", "0x1A2B3C", "x1A2B3C", "devbox"] {
            assert!(
                !normalised.contains(leaked),
                "{leaked} survived redaction: {normalised}"
            );
        }
        assert!(
            normalised.contains(REDACTED_PID)
                && normalised.contains(REDACTED_PROVIDER)
                && normalised.contains(REDACTED_PATH),
            "each rule must leave its placeholder behind: {normalised}"
        );
    }

    /// The control that makes the assertions above discriminate: the input
    /// really does carry the values, so a `normalise` that returned its input
    /// unchanged would fail rather than pass for want of anything to redact.
    #[test]
    fn the_redaction_input_carries_the_values_it_is_asserted_to_lose() {
        for present in ["pid:4812", "providerId:0x1A2B3C", r"\Users\devbox\"] {
            assert!(
                PROVIDER_DESCRIPTION.contains(present),
                "{present} is missing from the fixture, so redacting it proves nothing"
            );
        }
    }

    /// A provider chain: one entry per provider, each with its own process id,
    /// its own provider id and its own module path under a user profile. The
    /// second user name is the value a first-occurrence rule commits verbatim.
    const PROVIDER_CHAIN: &str = concat!(
        "[pid:4812,providerId:0x1A2B3C hwnd:0x90210 ",
        "Main(parent link):Microsoft: MSAA Proxy (unmanaged:C:",
        r"\Users\devbox\app\uiautomationcore.dll); ",
        "pid:991,providerId:0xFEED ",
        "Nonclient:Microsoft: Non-Client Proxy (unmanaged:D:",
        r"\Users\buildbot\host\uiautomationcore.dll)]"
    );

    /// No rule here is a first-occurrence rule. Every host value in the chain
    /// is redacted, including the *second* module path: a `redact_paths` that
    /// substituted only the first `\users\` would leave the later user name in
    /// a committed capture while the placeholder already present made the
    /// value read as redacted.
    #[test]
    fn every_substitution_redacts_each_occurrence_not_only_the_first() {
        let normalised = normalise(PROVIDER_CHAIN);

        for leaked in ["4812", "991", "FEED", "1A2B3C", "devbox", "buildbot"] {
            assert!(
                !normalised.contains(leaked),
                "a later occurrence of {leaked} survived: {normalised}"
            );
        }
        assert_eq!(
            normalised.matches(REDACTED_PATH).count(),
            2,
            "each profile path in the chain leaves its own placeholder: {normalised}"
        );
    }

    /// The control for the chain fixture: it really does carry two distinct
    /// user names, so asserting the second one absent proves the loop rather
    /// than passing for want of a second path.
    #[test]
    fn the_chain_input_carries_two_distinct_user_paths() {
        for present in [r"\Users\devbox\", r"\Users\buildbot\"] {
            assert!(
                PROVIDER_CHAIN.contains(present),
                "{present} is missing from the chain fixture, so redacting it proves nothing"
            );
        }
    }

    /// A string carrying none of the host values is returned intact, so the
    /// rules cannot pass the tests above by deleting text indiscriminately.
    #[test]
    fn a_string_with_nothing_to_redact_is_unchanged() {
        let harmless = "Main(parent link):Windows Presentation Foundation";
        assert_eq!(normalise(harmless), harmless);
    }

    #[test]
    fn text_presence_keeps_absent_and_unknown_distinct() {
        let absent = text_presence(&PropertyOutcome::Absent);
        let unknown = text_presence(&PropertyOutcome::Unknown);

        assert_ne!(absent, unknown);
        assert_eq!(absent, json!({ "present": false, "outcome": "<absent>" }));
        assert_eq!(unknown, json!({ "present": false, "outcome": "<unknown>" }));
    }

    #[test]
    fn field_presence_keeps_absent_and_unknown_distinct() {
        let absent = field_presence(&LocatorField::Absent);
        let unknown = field_presence(&LocatorField::Unknown);

        assert_ne!(absent, unknown);
        assert_eq!(absent, json!({ "present": false, "outcome": "<absent>" }));
        assert_eq!(unknown, json!({ "present": false, "outcome": "<unknown>" }));
    }
}
