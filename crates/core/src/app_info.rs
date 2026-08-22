use serde::{Deserialize, Serialize};

use crate::ProcessId;

/// How an application presents itself to the user, so an agent can tell a
/// window-owning app from one that only appears on a hotkey or lives in the
/// menu bar or tray.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppPresentation {
    /// Owns ordinary windows and appears in the Dock or taskbar.
    Foreground,
    /// No Dock or taskbar entry. Menu bar and tray items live here, as do
    /// overlays summoned by a hotkey; their windows may exist only while shown.
    Background,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppInfo {
    pub name: String,
    pub pid: ProcessId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process_instance: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub presentation: Option<AppPresentation>,
}

impl AppInfo {
    /// Whether `id` identifies this application, by exact case-insensitive
    /// name or bundle identifier. The one predicate every launch-target
    /// match uses, so a name match and a bundle match never drift apart.
    pub fn matches_identifier(&self, id: &str) -> bool {
        app_name_matches(&self.name, id)
            || self
                .bundle_id
                .as_deref()
                .is_some_and(|bundle_id| bundle_id.eq_ignore_ascii_case(id))
    }
}

pub fn app_name_matches(actual: &str, expected: &str) -> bool {
    let mut actual = actual
        .chars()
        .filter(|character| !crate::search_text::is_bidirectional_control(*character));
    let mut expected = expected
        .chars()
        .filter(|character| !crate::search_text::is_bidirectional_control(*character));
    loop {
        match (actual.next(), expected.next()) {
            (Some(actual), Some(expected)) if actual.eq_ignore_ascii_case(&expected) => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(name: &str, bundle_id: Option<&str>) -> AppInfo {
        AppInfo {
            name: name.to_owned(),
            pid: ProcessId::new(1),
            bundle_id: bundle_id.map(str::to_owned),
            process_instance: None,
            presentation: None,
        }
    }

    #[test]
    fn matches_by_exact_name() {
        assert!(app("Fixture", None).matches_identifier("Fixture"));
    }

    #[test]
    fn matches_by_bundle_id() {
        assert!(
            app("Fixture", Some("com.example.fixture")).matches_identifier("com.example.fixture")
        );
    }

    #[test]
    fn matches_are_case_insensitive() {
        assert!(app("Fixture", None).matches_identifier("fixture"));
        assert!(
            app("Fixture", Some("com.example.Fixture")).matches_identifier("COM.EXAMPLE.FIXTURE")
        );
    }

    #[test]
    fn matches_name_with_bidirectional_formatting_controls() {
        assert!(app("\u{200e}WhatsApp", None).matches_identifier("WhatsApp"));
        assert!(app("WhatsApp", None).matches_identifier("\u{2067}WhatsApp\u{2069}"));
    }

    #[test]
    fn name_identity_remains_exact_after_bidi_controls_are_removed() {
        assert!(!app("Foo  Bar", None).matches_identifier("Foo Bar"));
        assert!(!app("WhatsApp Beta", None).matches_identifier("WhatsApp"));
        assert!(!app("WhatsApp!", None).matches_identifier("WhatsApp"));
    }

    #[test]
    fn does_not_match_an_unrelated_identifier() {
        assert!(!app("Fixture", Some("com.example.fixture")).matches_identifier("Other"));
    }
}
