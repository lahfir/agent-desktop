use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationInfo {
    pub index: usize,
    pub app_name: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub actions: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct NotificationFilter {
    pub app: Option<String>,
    pub text: Option<String>,
    pub limit: Option<usize>,
}

/// Identity fingerprint used to verify that a notification-action call
/// targets the same notification a host observed via `list_notifications`.
///
/// Notification Center reindexes entries on every listing: opening,
/// dismissing, or receiving a new notification shifts what sits at any
/// given `index`. Calling `notification_action(index, ...)` without
/// verification can therefore press the action button on a different
/// notification than the host intended (confused-deputy at the OS
/// boundary).
///
/// The host should pass the `app_name` and/or `title` observed on the
/// notification at the moment of listing. If either field is `Some` and
/// the row currently at `index` does not match, the action is rejected
/// with `ErrorCode::NotificationNotFound`. Leaving both `None` preserves
/// the legacy index-only behavior for callers that have already done
/// their own reconciliation.
#[derive(Debug, Clone, Default)]
pub struct NotificationIdentity {
    pub expected_app: Option<String>,
    pub expected_title: Option<String>,
}

impl NotificationIdentity {
    pub fn is_empty(&self) -> bool {
        self.expected_app.is_none() && self.expected_title.is_none()
    }

    /// Returns true when `info` matches every field that is `Some` on
    /// this identity. `None` fields are treated as wildcards.
    pub fn matches(&self, info: &NotificationInfo) -> bool {
        if let Some(ref app) = self.expected_app {
            if app != &info.app_name {
                return false;
            }
        }
        if let Some(ref title) = self.expected_title {
            if title != &info.title {
                return false;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_info_serialization_omits_none_fields() {
        let info = NotificationInfo {
            index: 1,
            app_name: "Messages".into(),
            title: "New message".into(),
            body: None,
            actions: vec![],
        };
        let json = serde_json::to_value(&info).unwrap();
        assert!(!json.as_object().unwrap().contains_key("body"));
        assert!(!json.as_object().unwrap().contains_key("actions"));
    }

    #[test]
    fn notification_info_serialization_includes_present_fields() {
        let info = NotificationInfo {
            index: 2,
            app_name: "Slack".into(),
            title: "Channel update".into(),
            body: Some("New message in #general".into()),
            actions: vec!["Reply".into(), "Open".into()],
        };
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["body"], "New message in #general");
        assert_eq!(json["actions"], serde_json::json!(["Reply", "Open"]));
    }

    #[test]
    fn notification_filter_default_is_unfiltered() {
        let filter = NotificationFilter::default();
        assert!(filter.app.is_none());
        assert!(filter.text.is_none());
        assert!(filter.limit.is_none());
    }

    fn sample_info(app: &str, title: &str) -> NotificationInfo {
        NotificationInfo {
            index: 1,
            app_name: app.into(),
            title: title.into(),
            body: None,
            actions: vec![],
        }
    }

    #[test]
    fn identity_default_is_empty_and_matches_everything() {
        let id = NotificationIdentity::default();
        assert!(id.is_empty());
        assert!(id.matches(&sample_info("Messages", "Hi")));
        assert!(id.matches(&sample_info("Slack", "New")));
    }

    #[test]
    fn identity_with_only_app_matches_any_title_on_that_app() {
        let id = NotificationIdentity {
            expected_app: Some("Messages".into()),
            expected_title: None,
        };
        assert!(!id.is_empty());
        assert!(id.matches(&sample_info("Messages", "anything")));
        assert!(!id.matches(&sample_info("Slack", "anything")));
    }

    #[test]
    fn identity_with_only_title_matches_any_app_with_that_title() {
        let id = NotificationIdentity {
            expected_app: None,
            expected_title: Some("Meeting starting".into()),
        };
        assert!(id.matches(&sample_info("Calendar", "Meeting starting")));
        assert!(!id.matches(&sample_info("Calendar", "Reminder")));
    }

    #[test]
    fn identity_with_both_requires_both_to_match() {
        let id = NotificationIdentity {
            expected_app: Some("Calendar".into()),
            expected_title: Some("Meeting".into()),
        };
        assert!(id.matches(&sample_info("Calendar", "Meeting")));
        assert!(!id.matches(&sample_info("Calendar", "Other")));
        assert!(!id.matches(&sample_info("Other", "Meeting")));
    }
}
