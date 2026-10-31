use super::*;

fn classify_roles(roles: &[Option<&str>]) -> Result<bool, AdapterError> {
    classify(
        0,
        Deadline::standard().unwrap(),
        |index| Ok(roles[*index].map(str::to_owned)),
        |index| Ok((index + 1 < roles.len()).then_some(index + 1)),
        |application, menubar| {
            assert_eq!(roles[*application], Some("AXApplication"));
            assert_eq!(roles[*menubar], Some("AXMenuBar"));
            Ok(true)
        },
    )
}

#[test]
fn only_application_menu_ancestry_selects_the_press_only_chain() {
    for roles in [
        vec![
            Some("AXMenuItem"),
            Some("AXMenu"),
            Some("AXMenuBarItem"),
            Some("AXMenuBar"),
            Some("AXApplication"),
        ],
        vec![
            Some("AXMenuBarItem"),
            Some("AXMenuBar"),
            Some("AXApplication"),
        ],
    ] {
        let native = classify_roles(&roles).unwrap();
        assert!(native);
    }
    for roles in [
        vec![Some("AXMenuItem"), Some("AXMenu"), Some("AXWebArea")],
        vec![
            Some("AXMenuItem"),
            Some("AXMenu"),
            Some("AXMenuBar"),
            Some("AXWebArea"),
            Some("AXApplication"),
        ],
        vec![Some("AXMenuItem"), Some("AXMenu"), Some("AXWindow")],
        vec![Some("AXMenuItem"), Some("AXApplication")],
        vec![Some("AXMenuItem"), Some("AXMenu"), Some("AXApplication")],
        vec![Some("AXMenuItem")],
        vec![Some("AXButton")],
        vec![Some("")],
        vec![None],
        vec![Some("AXMenuItem"), None],
    ] {
        let native = classify_roles(&roles).unwrap();
        assert!(!native, "{roles:?}");
    }
}

#[test]
fn role_and_parent_failures_are_never_downgraded_to_ordinary_clicks() {
    for code in [
        ErrorCode::PermDenied,
        ErrorCode::Timeout,
        ErrorCode::StaleRef,
        ErrorCode::AppUnresponsive,
    ] {
        let error = AdapterError::new(code.clone(), "native read failed")
            .with_details(serde_json::json!({ "ax_error": -25202 }));
        let role_error = classify(
            0,
            Deadline::standard().unwrap(),
            |_| Err(error.clone()),
            |_| Ok(None),
            |_, _| panic!("no menubar"),
        )
        .unwrap_err();
        assert_eq!(role_error.code, code);
        assert_eq!(role_error.details, error.details);
        let parent_error = classify(
            0,
            Deadline::standard().unwrap(),
            |_| Ok(Some("AXMenuItem".into())),
            |_| Err(error.clone()),
            |_, _| panic!("no menubar"),
        )
        .unwrap_err();
        assert_eq!(parent_error.code, code);
        assert_eq!(parent_error.details, error.details);
    }
}

#[test]
fn ancestry_cycles_exhaust_the_budget_instead_of_enabling_fallbacks() {
    let error = classify(
        0,
        Deadline::standard().unwrap(),
        |_| Ok(Some("AXMenuItem".into())),
        |_| Ok(Some(0)),
        |_, _| panic!("cycle has no app"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::AppUnresponsive);
    assert_eq!(error.details.unwrap()["complete"], false);
}

#[test]
fn exhausted_deadline_never_reads_or_selects_a_click_chain() {
    let deadline = Deadline::after(1).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(2));
    let error = classify(
        0,
        deadline,
        |_| panic!("role read after deadline"),
        |_| panic!("parent read after deadline"),
        |_, _| panic!("provenance read after deadline"),
    )
    .unwrap_err();
    assert_eq!(error.code, ErrorCode::Timeout);
}

#[test]
fn menu_roles_cannot_substitute_for_the_applications_actual_menubar() {
    for ownership in [
        Ok(false),
        Err(AdapterError::new(
            ErrorCode::PermDenied,
            "cannot read application menubar",
        )),
    ] {
        let result = classify(
            0,
            Deadline::standard().unwrap(),
            |index| {
                Ok(Some(
                    ["AXMenuBarItem", "AXMenuBar", "AXApplication"][*index].into(),
                ))
            },
            |index| Ok(Some(index + 1)),
            |_, _| ownership.clone(),
        );
        match ownership {
            Ok(false) => assert!(!result.unwrap()),
            Err(error) => assert_eq!(result.unwrap_err().code, error.code),
            _ => unreachable!(),
        }
    }
}
