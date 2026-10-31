use super::*;
use std::time::Duration;

fn surface(id: &str, kind: &str) -> SurfaceInfo {
    SurfaceInfo {
        id: id.into(),
        kind: kind.into(),
        title: Some(format!("{kind} title")),
        item_count: Some(3),
        unclassified: Vec::new(),
    }
}

#[test]
fn shared_menu_discovery_preserves_other_surfaces_and_existing_menu_ids() {
    let window_surfaces = vec![
        surface("focused-window", "sheet"),
        surface("focused-window/children/2", "popover"),
        surface("focused-window/children/4", "alert"),
    ];
    for menus in [
        vec![],
        vec![surface("app/menubar/children/1/children/0", "menu")],
    ] {
        let mut expected = menus.clone();
        expected.extend(window_surfaces.clone());
        let actual = list_surfaces_with(
            Instant::now() + Duration::from_secs(1),
            || Ok(window_surfaces.clone()),
            || Ok(menus),
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
    }
}

#[test]
fn incomplete_shared_discovery_is_not_reported_as_an_empty_inventory() {
    for failed_menu in [true, false] {
        let read = |fail| {
            if fail {
                Err(
                    AdapterError::new(agent_desktop_core::ErrorCode::AppUnresponsive, "incomplete")
                        .with_details(
                            serde_json::json!({ "complete": false, "attribute": "AXChildren" }),
                        ),
                )
            } else {
                Ok(vec![surface("observed", "sheet")])
            }
        };
        let error = list_surfaces_with(
            Instant::now() + Duration::from_secs(1),
            || read(!failed_menu),
            || read(failed_menu),
        )
        .unwrap_err();
        let details = error.details.unwrap();
        assert_eq!(details["complete"], false);
        assert_eq!(details["attribute"], "AXChildren");
    }
}

#[test]
fn expired_deadline_fails_before_accessibility_inventory() {
    let deadline = Instant::now() - Duration::from_millis(1);
    let pid = i32::try_from(std::process::id()).expect("test pid fits macOS pid_t");
    let identity = crate::system::process::process_identity::ProcessIdentity::capture(pid)
        .unwrap()
        .unwrap();
    let error =
        list_surfaces_for_process(identity, deadline).expect_err("expired surface inventory");
    assert_eq!(error.code.as_str(), "TIMEOUT");
}
