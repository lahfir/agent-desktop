use super::*;

#[test]
fn post_action_wait_without_flag_returns_action_only() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    refmap.allocate(entry());
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = ScopedWaitAdapter {
        request: Mutex::new(None),
        polled_app: Mutex::new(None),
        lease_held: Arc::new(AtomicBool::new(false)),
    };
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
        timeout_ms: None,
    };

    let value = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &CommandContext::default(),
    )
    .unwrap();

    assert_eq!(value["action"], "ok");
    assert!(value.get("after_action").is_none());
}

#[test]
fn post_action_wait_timeout_embeds_action_result_in_details() {
    let _guard = HomeGuard::new();
    let mut refmap = RefMap::new();
    let mut entry = entry();
    entry.source.source_app = Some("TargetApp".into());
    refmap.allocate(entry);
    let snapshot_id = RefStore::new().unwrap().save_new_snapshot(&refmap).unwrap();
    let adapter = ScopedWaitAdapter {
        request: Mutex::new(None),
        polled_app: Mutex::new(None),
        lease_held: Arc::new(AtomicBool::new(false)),
    };
    let context = CommandContext::default().with_wait_selector(Some(WaitSelector {
        query_raw: ":never-appears".into(),
        gone: false,
        timeout_ms: 50,
    }));
    let args = RefArgs {
        ref_id: "@e1".into(),
        snapshot_id: Some(snapshot_id),
        timeout_ms: None,
    };

    let err = execute_ref_action_with_context(
        args,
        &adapter,
        ActionRequest::headless(Action::Click),
        &context,
    )
    .unwrap_err();

    assert_eq!(err.code(), "TIMEOUT");
    let details = match err {
        crate::AppError::Adapter(adapter_err) => adapter_err.details.expect("details"),
        other => panic!("expected adapter timeout, got {other:?}"),
    };
    assert_eq!(details["kind"], "wait_timeout");
    assert_eq!(details["after_action"]["action"], "ok");
}
