use agent_desktop_core::{ErrorCode, InteractionPolicy, NotificationIdentity, NotificationInfo};

use super::{require_foreground_policy, verify_identity};

fn info(app: &str, title: &str) -> NotificationInfo {
    NotificationInfo {
        index: 1,
        app_name: app.into(),
        title: title.into(),
        body: None,
        actions: Vec::new(),
    }
}

#[test]
fn an_empty_identity_defers_to_the_index() {
    let identity = NotificationIdentity::default();

    assert!(verify_identity(1, Some(&identity), &info("App A", "Reminder")).is_ok());
    assert!(verify_identity(1, None, &info("App A", "Reminder")).is_ok());
}

#[test]
fn a_matching_identity_is_accepted() {
    let identity = NotificationIdentity {
        expected_app: Some("App A".into()),
        expected_title: Some("Reminder".into()),
    };

    assert!(verify_identity(1, Some(&identity), &info("App A", "Reminder")).is_ok());
}

#[test]
fn a_mismatched_identity_is_notification_not_found_and_names_only_the_index() {
    let identity = NotificationIdentity {
        expected_app: Some("App A".into()),
        expected_title: Some("Reminder".into()),
    };

    let error = verify_identity(2, Some(&identity), &info("App B", "Moved in underneath"))
        .expect_err("the surface reordered under the caller");

    assert_eq!(error.code, ErrorCode::NotificationNotFound);
    assert!(error.message.contains("index 2"));
    assert!(
        !error.message.contains("App B"),
        "the mismatch message is built from the index alone and must never carry the entry that actually sits there"
    );
    assert!(!error.message.contains("Moved in underneath"));
}

#[test]
fn a_title_only_identity_is_verified_against_the_title() {
    let identity = NotificationIdentity {
        expected_app: None,
        expected_title: Some("Reminder".into()),
    };

    assert!(verify_identity(1, Some(&identity), &info("App A", "Reminder")).is_ok());
    assert!(verify_identity(1, Some(&identity), &info("App A", "Other")).is_err());
}

#[test]
fn a_strict_headless_caller_is_policy_denied_before_anything_raises() {
    let error = require_foreground_policy(InteractionPolicy::headless())
        .expect_err("headless policy must refuse");

    assert_eq!(error.code, ErrorCode::PolicyDenied);
}

#[test]
fn a_policy_that_permits_the_foreground_change_proceeds() {
    assert!(require_foreground_policy(InteractionPolicy::focus_fallback()).is_ok());
    assert!(require_foreground_policy(InteractionPolicy::headed()).is_ok());
}
