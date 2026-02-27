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
}
