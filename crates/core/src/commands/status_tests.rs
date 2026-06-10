use super::*;
use crate::PermissionState;

struct DeniedAdapter;

impl PlatformAdapter for DeniedAdapter {
    fn permission_report(&self) -> PermissionReport {
        PermissionReport {
            accessibility: PermissionState::Denied {
                suggestion: "should not be used".into(),
            },
            screen_recording: PermissionState::Denied {
                suggestion: "should not be used".into(),
            },
            automation: PermissionState::Unknown,
        }
    }
}

#[test]
fn status_uses_precomputed_permission_report() {
    let report = PermissionReport {
        accessibility: PermissionState::Granted,
        screen_recording: PermissionState::Granted,
        automation: PermissionState::NotRequired,
    };

    let value =
        execute_with_report_with_context(&DeniedAdapter, &report, &CommandContext::default())
            .unwrap();
    let permissions = value.get("permissions").unwrap();

    assert_eq!(permissions["accessibility"]["state"], "granted");
    assert_eq!(permissions["screen_recording"]["state"], "granted");
    assert_eq!(permissions["automation"]["state"], "not_required");
}
