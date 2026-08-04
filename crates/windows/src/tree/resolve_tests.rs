    use super::*;
    use agent_desktop_core::{ElementIdentifier, LocatorEvidence, RefEntry, WindowInfo};

    fn first_identifier(evidence: &LocatorEvidence) -> Option<ElementIdentifier> {
        evidence
            .identifiers
            .identifiers()
            .iter()
            .find(|identifier| {
                matches!(identifier.kind, agent_desktop_core::IdentifierKind::AutomationId)
            })
            .cloned()
    }

    fn capture_identified(
        root: &UIAElement,
        deadline: agent_desktop_core::Deadline,
    ) -> Option<(Option<ElementIdentifier>, Option<String>, Option<String>)> {
        let source = UiaTreeSource::for_root(root).ok()?;
        let prepared = source.prepare_root(root).ok()?;
        let budget = WalkBudget::new(10, deadline);
        walk_for_identity(&source, &prepared, 0, &budget)
    }

    fn walk_for_identity(
        source: &UiaTreeSource,
        element: &UIAElement,
        depth: u8,
        budget: &WalkBudget,
    ) -> Option<(Option<ElementIdentifier>, Option<String>, Option<String>)> {
        if depth >= 10 {
            return None;
        }
        let (_, evidence, _) = source.evidence(element);
        let native_id = first_identifier(&evidence);
        if native_id.is_some() {
            return Some((
                native_id,
                evidence.role.known().cloned(),
                evidence.name.known().cloned(),
            ));
        }
        let mut ignored = false;
        let children = crate::tree::resolve_search::enumerate_children(source, element, budget, &mut ignored)
            .ok()?;
        for child in children {
            if let Some(found) = walk_for_identity(source, &child, depth + 1, budget) {
                return Some(found);
            }
        }
        None
    }

    #[test]
    fn a_fixture_ref_resolves_to_the_same_element_end_to_end() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
        let window = WindowInfo {
            id: format!("w-{}", fixture.handle()),
            title: "agent-desktop fixture".into(),
            app: "fixture.exe".into(),
            pid: agent_desktop_core::ProcessId::from(fixture.process_id()),
            process_instance: Some(
                crate::system::process_identity::token_for_pid(
                    agent_desktop_core::ProcessId::from(fixture.process_id()),
                )
                .unwrap()
                .expect("a live fixture process has a token"),
            ),
            bounds: None,
            state: Default::default(),
        };
        let deadline = crate::tree::walker_fake::deadline();
        let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline)
            .expect("the fixture window resolves");
        let token = window.process_instance.clone().unwrap();

        let captured = capture_identified(&root, deadline).expect("a fixture element has an id");

        let entry = RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: window.pid,
                process_instance: Some(token),
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: captured.1.clone().unwrap_or_default(),
                name: captured.2.clone(),
                value: None,
                description: None,
                native_id: captured.0.clone(),
            },
            geometry: agent_desktop_core::RefGeometry {
                bounds: None,
                bounds_hash: None,
            },
            capabilities: agent_desktop_core::RefCapabilities {
                states: Vec::new(),
                available_actions: Vec::new(),
            },
            source: agent_desktop_core::RefSource {
                source_app: Some("fixture.exe".into()),
                source_window_id: Some(window.id.clone()),
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: false,
                path: agent_desktop_core::refs::RefPath::default(),
            },
        };

        let handle = resolve_element_strict(&entry, deadline)
            .expect("the stored identity re-resolves to a live element");

        assert!(
            handle.downcast_ref::<UIAElement>().is_some(),
            "the resolved handle carries a UI Automation element"
        );
    }

    /// A ref taken from the fixture's password control - no text identity,
    /// positive-area bounds, secure content withheld - resolves through the
    /// path fast-path and the geometry tier on an unchanged tree, and the
    /// secure value reaches no error or detail.
    #[test]
    fn a_blank_secure_ref_resolves_through_the_path_and_geometry_tier() {
        crate::tree::fixture::ensure_test_apartment();
        let fixture = crate::tree::fixture::HostedFixture::spawn().expect("a fixture host starts");
        let deadline = crate::tree::walker_fake::deadline();
        let root = crate::tree::automation::root_from_hwnd(fixture.handle(), deadline)
            .expect("the fixture window resolves");
        let source = UiaTreeSource::for_root(&root).expect("a tree source");
        let prepared = source.prepare_root(&root).expect("a prepared root");
        let budget = WalkBudget::new(10, deadline);

        // Locate the password edit: an unlabelled, id-less EDIT whose value is
        // withheld by the secure gate, and record its child-index path.
        let mut prefix = Vec::new();
        let found = find_password(
            &source,
            &prepared,
            0,
            &budget,
            &mut prefix,
        )
        .expect("the fixture exposes a password edit")
        .expect("a password element");
        let (path, _, evidence, _) = found;
        let role = evidence.role.known().cloned();
        let rect = evidence.ref_evidence.bounds.known().expect("a bounds");
        let hash = rect.bounds_hash().expect("a positive-area hash");

        let entry = RefEntry {
            process: agent_desktop_core::RefProcess {
                pid: agent_desktop_core::ProcessId::from(fixture.process_id()),
                process_instance: Some(
                    crate::system::process_identity::token_for_pid(
                        agent_desktop_core::ProcessId::from(fixture.process_id()),
                    )
                    .unwrap()
                    .expect("a live fixture process has a token"),
                ),
            },
            identity: agent_desktop_core::RefEntryIdentity {
                role: role.clone().unwrap_or_default(),
                name: None,
                value: None,
                description: None,
                native_id: None,
            },
            geometry: agent_desktop_core::RefGeometry {
                bounds: Some(*rect),
                bounds_hash: Some(hash),
            },
            capabilities: agent_desktop_core::RefCapabilities {
                states: Vec::new(),
                available_actions: Vec::new(),
            },
            source: agent_desktop_core::RefSource {
                source_app: Some("fixture.exe".into()),
                source_window_id: Some(format!("w-{}", fixture.handle())),
                source_window_title: None,
                source_window_bounds_hash: None,
                source_surface: agent_desktop_core::SnapshotSurface::Window,
            },
            scope: agent_desktop_core::RefScope {
                root_ref: None,
                path_is_absolute: true,
                path,
            },
        };

        assert!(
            crate::tree::resolve_search::can_use_path_fast_path(&entry),
            "a window-rooted path with a positive-area hash qualifies"
        );
        assert!(
            crate::tree::resolve_search::provisional_geometry_candidate(&entry),
            "no meaningful identity plus a positive-area hash is promotion-eligible"
        );

        let handle = resolve_element_strict(&entry, deadline)
            .expect("the blank secure ref resolves through path and geometry");

        assert!(
            handle.downcast_ref::<UIAElement>().is_some(),
            "the resolved handle carries a UI Automation element"
        );
    }

    fn find_password(
        source: &UiaTreeSource,
        element: &UIAElement,
        depth: u8,
        budget: &WalkBudget,
        prefix: &mut Vec<usize>,
    ) -> Result<
        Option<(
            agent_desktop_core::refs::RefPath,
            crate::tree::properties::ElementProperties,
            LocatorEvidence,
            u64,
        )>,
        AdapterError,
    > {
        if depth >= 10 {
            return Ok(None);
        }
        let (properties, node_evidence, failed) = source.evidence(element);
        if properties.is_secure() {
            let mut path = agent_desktop_core::refs::RefPath::default();
            path.extend_from_slice(prefix);
            return Ok(Some((path, properties, node_evidence, failed)));
        }
        let mut ignored = false;
        let children = crate::tree::resolve_search::enumerate_children(source, element, budget, &mut ignored)?;
        for (index, child) in children.iter().enumerate() {
            prefix.push(index);
            if let Some(found) = find_password(source, child, depth + 1, budget, prefix)? {
                return Ok(Some(found));
            }
            prefix.pop();
        }
        Ok(None)
    }

    fn retryable_incomplete(message: &str) -> AdapterError {
        AdapterError::new(agent_desktop_core::ErrorCode::AppUnresponsive, message).with_details(
            serde_json::json!({ "retryable": true, "complete": false }),
        )
    }

    fn err_code(result: Result<NativeHandle, AdapterError>) -> ErrorCode {
        match result {
            Err(error) => error.code,
            Ok(_) => panic!("expected an error, got a resolved handle"),
        }
    }

    fn err_code_owned(result: Result<NativeHandle, AdapterError>) -> AdapterError {
        match result {
            Err(error) => error,
            Ok(_) => panic!("expected an error, got a resolved handle"),
        }
    }

    fn short_deadline() -> Deadline {
        Deadline::after(200).expect("a deadline")
    }

    fn generous_deadline() -> Deadline {
        Deadline::after(5_000).expect("a deadline")
    }

    /// An incomplete attempt retries within its deadline and succeeds when the
    /// underlying cause recovers - the tree stabilises after a vanishing
    /// node, the fake recovers.
    #[test]
    fn an_incomplete_attempt_retries_and_succeeds_within_the_deadline() {
        let mut attempts = 0;
        let deadline = generous_deadline();
        let result = retry_incomplete_until(deadline, || {
            attempts += 1;
            if attempts < 3 {
                Err(retryable_incomplete("transient"))
            } else {
                Ok(unreachable_handle())
            }
        });
        assert!(result.is_ok());
        assert_eq!(attempts, 3);
    }

    /// A settled non-match (a completed search that finds nothing) is never
    /// retried - the call-count pin fails if the classification is loosened.
    #[test]
    fn a_settled_non_match_never_retries() {
        let mut attempts = 0;
        let deadline = generous_deadline();
        let result = retry_incomplete_until(deadline, || {
            attempts += 1;
            Err(agent_desktop_core::AdapterError::stale_ref("nothing").with_details(
                serde_json::json!({ "complete": true, "retryable": true }),
            ))
        });
        assert_eq!(err_code(result), agent_desktop_core::ErrorCode::StaleRef);
        assert_eq!(attempts, 1);
    }

    /// An unresponsive error that was not stamped retryable is not retried: the
    /// loop's `is_retryable_resolution_error` requires the explicit stamp, so a
    /// raw transport error terminates rather than burning the budget guessing.
    #[test]
    fn an_unstamped_unresponsive_error_is_not_retried() {
        let mut attempts = 0;
        let deadline = generous_deadline();
        let result = retry_incomplete_until(deadline, || {
            attempts += 1;
            Err(AdapterError::new(
                agent_desktop_core::ErrorCode::AppUnresponsive,
                "raw",
            ))
        });
        assert_eq!(err_code(result), agent_desktop_core::ErrorCode::AppUnresponsive);
        assert_eq!(attempts, 1);
    }

    /// Deadline expiry mid-incompleteness returns the last incomplete
    /// diagnosis stamped `deadline_elapsed`, not a bare `TIMEOUT` that
    /// discards the diagnosis.
    #[test]
    fn deadline_expiry_mid_incompleteness_returns_the_last_diagnosis_stamped() {
        let deadline = short_deadline();
        let result = retry_incomplete_until(deadline, || Err(retryable_incomplete("stuck")));
        let error = err_code_owned(result);
        assert_eq!(error.code, agent_desktop_core::ErrorCode::AppUnresponsive);
        assert_eq!(
            error
                .details
                .as_ref()
                .and_then(|details| details.get("deadline_elapsed"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    /// Expiry with no incomplete diagnosis returns the plain timeout - there
    /// is nothing more informative to surface.
    #[test]
    fn deadline_expiry_with_no_incomplete_returns_the_timeout() {
        let deadline = short_deadline();
        let result = retry_incomplete_until(deadline, || {
            Err(AdapterError::new(agent_desktop_core::ErrorCode::Timeout, "gone"))
        });
        assert_eq!(err_code(result), agent_desktop_core::ErrorCode::Timeout);
    }

    /// The deadline stamp and the typed details fields are the only places a
    /// marker survives; message and `platform_detail` stay clean.
    #[test]
    fn the_deadline_stamp_leaks_no_marker_into_message_or_platform_detail() {
        let error = AdapterError::new(
            agent_desktop_core::ErrorCode::AppUnresponsive,
            "Strict resolution could not determine candidate identity",
        )
        .with_platform_detail("com-hresult-shape")
        .with_details(serde_json::json!({
            "kind": "resolution_identity_unknown",
            "secret_slot": "MARKER-9f2c",
            "complete": false,
            "retryable": true,
        }));

        let stamped = mark_deadline_elapsed(error);
        assert!(!stamped.message.contains("MARKER-9f2c"));
        assert!(!stamped.platform_detail.unwrap_or_default().contains("MARKER-9f2c"));
        let details = stamped.details.expect("details preserved");
        assert_eq!(
            details.get("secret_slot").and_then(serde_json::Value::as_str),
            Some("MARKER-9f2c")
        );
        assert_eq!(
            details.get("deadline_elapsed").and_then(serde_json::Value::as_bool),
            Some(true)
        );
    }

    fn unreachable_handle() -> NativeHandle {
        NativeHandle::new(())
    }
