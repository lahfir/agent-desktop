use super::*;

struct StrictDiagnosticAdapter;

impl ObservationOps for StrictDiagnosticAdapter {}
impl ActionOps for StrictDiagnosticAdapter {}
impl InputOps for StrictDiagnosticAdapter {}
impl SystemOps for StrictDiagnosticAdapter {
    fn resolve_window_strict(
        &self,
        expected: &WindowInfo,
        _deadline: crate::Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        if expected.id == "w-1"
            && expected.pid == crate::ProcessId::new(1)
            && expected.process_instance.as_deref() == Some("test-instance")
        {
            Ok(expected.clone())
        } else {
            Err(AdapterError::new(
                ErrorCode::WindowNotFound,
                "Original window is absent",
            ))
        }
    }
}

#[test]
fn ref_wait_timeout_diagnostics_reject_other_windows_and_process_generations() {
    let _home = HomeGuard::new();
    let adapter = StrictDiagnosticAdapter;
    for (window_id, generation) in [("other-window", "test-instance"), ("w-1", "old-instance")] {
        let mut entry = crate::adapter::minimal_ref_entry("button", None);
        entry.source.source_window_id = Some(window_id.into());
        entry.process.process_instance = Some(generation.into());
        let error = timeout_response(
            &adapter,
            &base_input("button:saved", false),
            Some(&entry),
            &CommandContext::default(),
            (None, None),
        )
        .unwrap_err();
        let AppError::Adapter(error) = error else {
            panic!("expected adapter error")
        };
        assert_eq!(error.code, ErrorCode::Timeout);
        let details = error.details.unwrap();
        assert!(details.get("snapshot_id").is_none());
        assert_eq!(details["last_error"]["code"], "WINDOW_NOT_FOUND");
    }
}

#[test]
fn unsupported_strict_resolution_does_not_fall_back_to_window_listing() {
    let _home = HomeGuard::new();
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.source.source_window_id = Some("w-1".into());
    let error = execute_with_ref(
        WaitSelectorInput {
            timeout_ms: 30,
            ..base_input("button:Close", true)
        },
        Some(&entry),
        &CodeErrorAdapter {
            code: ErrorCode::WindowNotFound,
        },
        &CommandContext::default(),
    )
    .unwrap_err();
    assert_eq!(error.code(), "PLATFORM_NOT_SUPPORTED");
    assert!(error.to_string().contains("resolve_window_strict"));
}

#[test]
fn confirmed_window_absence_survives_default_process_state_capability() {
    let _home = HomeGuard::new();
    let mut entry = crate::adapter::minimal_ref_entry("button", None);
    entry.source.source_window_id = Some("closed-window".into());
    let result = execute_with_ref(
        base_input("button:Close", true),
        Some(&entry),
        &StrictDiagnosticAdapter,
        &CommandContext::default(),
    )
    .unwrap();
    assert_eq!(result["target_absent"], true);
}
