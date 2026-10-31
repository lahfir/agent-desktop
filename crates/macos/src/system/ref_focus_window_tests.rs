use super::*;

#[test]
fn live_window_recovery_uses_exact_attribute_or_ancestor() {
    for direct in [true, false] {
        let window = find_window(
            0,
            |node| {
                Ok(Some(
                    if *node == 2 { "AXWindow" } else { "AXMenuItem" }.into(),
                ))
            },
            |node, attr| {
                Ok(match (*node, attr, direct) {
                    (0, "AXWindow", true) => Some(2),
                    (0, "AXParent", false) => Some(1),
                    (1, "AXWindow", false) => Some(2),
                    _ => None,
                })
            },
        )
        .unwrap();
        assert_eq!(window, Some(2));
    }
}

#[test]
fn missing_unknown_or_cyclic_ownership_cannot_select_a_window() {
    assert_eq!(find_window(0, |_| Ok(None), |_, _| Ok(None)).unwrap(), None);
    assert!(find_window(0, |_| Ok(Some("AXMenu".into())), |_, _| Ok(Some(1))).is_err());
    assert!(
        find_window(
            0,
            |_| Ok(None),
            |_, attr| Ok((attr == "AXParent").then_some(0))
        )
        .is_err()
    );
}

#[test]
fn ownership_reads_preserve_native_failures() {
    for code in [
        ErrorCode::PermDenied,
        ErrorCode::Timeout,
        ErrorCode::StaleRef,
    ] {
        let error = find_window(
            0,
            |_| Ok(None),
            |_, _| Err(AdapterError::new(code.clone(), "read failed")),
        )
        .unwrap_err();
        assert_eq!(error.code, code);
    }
}
