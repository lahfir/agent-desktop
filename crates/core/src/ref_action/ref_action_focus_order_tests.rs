use crate::{
    Action, ActionOps, ActionRequest, ActionResult, AdapterError, Deadline, ElementState, InputOps,
    InteractionLease, LiveElement, NativeHandle, ObservationOps, RefEntry, SystemOps, WindowInfo,
};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

struct FocusAdapter {
    focused: AtomicBool,
    dispatched: AtomicUsize,
    resolved: AtomicUsize,
    persistent_occlusion: bool,
    enabled: bool,
    application_menu: bool,
}

fn bounds() -> crate::Rect {
    crate::Rect {
        x: 1.0,
        y: 1.0,
        width: 20.0,
        height: 20.0,
    }
}

impl FocusAdapter {
    fn live_bounds(&self) -> crate::Rect {
        crate::Rect {
            x: if self.focused.load(Ordering::SeqCst) {
                100.0
            } else {
                1.0
            },
            ..bounds()
        }
    }
}

impl ObservationOps for FocusAdapter {
    fn resolve_element_strict(
        &self,
        _: &RefEntry,
        _: Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        self.resolved.fetch_add(1, Ordering::SeqCst);
        Ok(NativeHandle::null())
    }

    fn get_live_element(&self, _: &NativeHandle, _: Deadline) -> Result<LiveElement, AdapterError> {
        Ok(LiveElement {
            identity: crate::adapter::live_identity("Run"),
            state: ElementState {
                role: if self.application_menu {
                    "menuitem"
                } else {
                    "button"
                }
                .into(),
                states: vec![],
                value: None,
                enabled: Some(self.enabled),
                hidden: Some(false),
                offscreen: (!self.application_menu).then_some(false),
            },
            states_complete: true,
            bounds: Some(if self.application_menu {
                crate::Rect {
                    width: 0.0,
                    height: 0.0,
                    ..bounds()
                }
            } else {
                self.live_bounds()
            }),
            available_actions: vec![crate::actions::capability::CLICK.into()],
        })
    }

    fn get_element_bounds(
        &self,
        _: &NativeHandle,
        _: Deadline,
    ) -> Result<Option<crate::Rect>, AdapterError> {
        Ok(Some(self.live_bounds()))
    }

    fn hit_test(
        &self,
        _: &NativeHandle,
        _: crate::Point,
        _: Deadline,
    ) -> Result<crate::HitTestResult, AdapterError> {
        assert!(
            !self.application_menu,
            "semantic menus must not hit-test a pointer"
        );
        Ok(
            if self.focused.load(Ordering::SeqCst) && !self.persistent_occlusion {
                crate::HitTestResult::ReachesTarget
            } else {
                crate::HitTestResult::InterceptedBy {
                    role: Some("window".into()),
                    name: None,
                    bounds: None,
                }
            },
        )
    }
}

impl ActionOps for FocusAdapter {
    fn headed_requirement_for_ref(
        &self,
        _: &RefEntry,
        request: &ActionRequest,
        deadline: Deadline,
    ) -> Result<crate::HeadedRequirement, AdapterError> {
        self.headed_requirement(&NativeHandle::null(), request, deadline)
    }

    fn headed_requirement(
        &self,
        _: &NativeHandle,
        request: &ActionRequest,
        _: Deadline,
    ) -> Result<crate::HeadedRequirement, AdapterError> {
        Ok(if self.application_menu && request.policy.is_headed() {
            crate::HeadedRequirement::FocusedApplication
        } else {
            request.headed_requirement()
        })
    }

    fn execute_action(
        &self,
        _: &NativeHandle,
        request: ActionRequest,
        _: &InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        if request.policy.is_headed() {
            assert!(self.focused.load(Ordering::SeqCst));
            assert_eq!(request.verified_point().map(|point| point.x), Some(110.0));
        }
        if self.application_menu {
            assert_eq!(request.policy, crate::InteractionPolicy::headless());
            assert!(request.verified_point().is_none());
        }
        self.dispatched.fetch_add(1, Ordering::SeqCst);
        Ok(ActionResult::delivered_unverified("click"))
    }
}
impl InputOps for FocusAdapter {}
impl SystemOps for FocusAdapter {
    crate::adapter::guarded_interaction_lease!();

    fn resolve_window_strict(
        &self,
        window: &WindowInfo,
        _: Deadline,
    ) -> Result<WindowInfo, AdapterError> {
        Ok(window.clone())
    }
    fn focus_app(
        &self,
        process: crate::ProcessIdentity,
        _: &InteractionLease,
    ) -> Result<(), AdapterError> {
        assert!(self.application_menu);
        assert_eq!(process, crate::ProcessIdentity::new(1, "test-instance"));
        self.focused.store(true, Ordering::SeqCst);
        Ok(())
    }

    fn focus_window(&self, _: &WindowInfo, _: &InteractionLease) -> Result<(), AdapterError> {
        assert!(
            !self.application_menu,
            "application menus have no document window"
        );
        self.focused.store(true, Ordering::SeqCst);
        Ok(())
    }
}

fn execute(adapter: &FocusAdapter, headed: bool) -> Result<ActionResult, AdapterError> {
    execute_within(adapter, headed, 2_000)
}

fn execute_within(
    adapter: &FocusAdapter,
    headed: bool,
    timeout_ms: u64,
) -> Result<ActionResult, AdapterError> {
    let _clock = crate::wait::deadline::VirtualClock::freeze();
    let mut entry: RefEntry = serde_json::from_value(serde_json::json!({
        "pid": 1, "process_instance": "test-instance", "role": "button", "name": "Run",
        "source_window_id": "w-test", "source_surface": "window", "path": [0],
        "bounds": bounds(), "bounds_hash": bounds().bounds_hash(), "available_actions": ["Click"], "states": []
    })).unwrap();
    if adapter.application_menu {
        entry.identity.role = "menuitem".into();
        entry.source.source_window_id = None;
        entry.source.source_surface = crate::SnapshotSurface::Menubar;
        entry.geometry.bounds_hash = None;
    }
    let request = if headed {
        ActionRequest::headed(Action::Click)
    } else {
        ActionRequest::headless(Action::Click)
    };
    crate::ref_action::execute_entry(adapter, &entry, request.with_timeout_ms(Some(timeout_ms)))
}

fn adapter(persistent_occlusion: bool, enabled: bool) -> FocusAdapter {
    FocusAdapter {
        focused: AtomicBool::new(false),
        dispatched: AtomicUsize::new(0),
        resolved: AtomicUsize::new(0),
        persistent_occlusion,
        enabled,
        application_menu: false,
    }
}

#[test]
fn headed_background_target_is_focused_before_final_hit_test_and_dispatch() {
    let adapter = adapter(false, true);
    execute(&adapter, true).unwrap();
    assert!(adapter.focused.load(Ordering::SeqCst));
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 1);
}

#[test]
fn headed_target_still_occluded_after_focus_never_receives_input() {
    let adapter = adapter(true, true);
    execute(&adapter, true).unwrap_err();
    assert!(adapter.focused.load(Ordering::SeqCst));
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 0);
}

#[test]
fn disabled_target_does_not_trigger_early_focus() {
    let adapter = adapter(false, false);
    execute_within(&adapter, true, 150).unwrap_err();
    assert!(!adapter.focused.load(Ordering::SeqCst));
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 0);
}

#[test]
fn headless_semantic_action_never_raises_the_window() {
    let adapter = adapter(false, true);
    execute(&adapter, false).unwrap();
    assert!(!adapter.focused.load(Ordering::SeqCst));
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 1);
}

#[test]
fn headed_application_menu_uses_semantic_actionability_before_application_focus() {
    let mut adapter = adapter(true, true);
    adapter.application_menu = true;
    execute(&adapter, true).unwrap();
    assert!(adapter.focused.load(Ordering::SeqCst));
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 1);
}

#[test]
fn headless_application_menu_never_activates_the_application() {
    let mut adapter = adapter(true, true);
    adapter.application_menu = true;
    execute(&adapter, false).unwrap();
    assert!(!adapter.focused.load(Ordering::SeqCst));
    assert_eq!(adapter.dispatched.load(Ordering::SeqCst), 1);
}

#[test]
fn headed_click_does_not_resolve_before_window_focus_and_dispatch() {
    struct DefaultRequirementAdapter(FocusAdapter);
    impl ActionOps for DefaultRequirementAdapter {}
    impl InputOps for DefaultRequirementAdapter {}
    impl ObservationOps for DefaultRequirementAdapter {
        fn resolve_element_strict(
            &self,
            entry: &RefEntry,
            deadline: Deadline,
        ) -> Result<NativeHandle, AdapterError> {
            self.0.resolve_element_strict(entry, deadline)
        }
    }
    impl SystemOps for DefaultRequirementAdapter {
        fn resolve_window_strict(
            &self,
            window: &WindowInfo,
            deadline: Deadline,
        ) -> Result<WindowInfo, AdapterError> {
            self.0.resolve_window_strict(window, deadline)
        }
        fn focus_window(
            &self,
            window: &WindowInfo,
            lease: &InteractionLease,
        ) -> Result<(), AdapterError> {
            self.0.focus_window(window, lease)
        }
    }
    let adapter = DefaultRequirementAdapter(adapter(false, true));
    let entry: RefEntry = serde_json::from_value(serde_json::json!({
        "pid": 1, "process_instance": "test-instance", "role": "button", "name": "Run",
        "source_window_id": "w-test", "source_surface": "window", "path": [0],
        "bounds": bounds(), "available_actions": ["Click"], "states": []
    }))
    .unwrap();
    let context = crate::CommandContext::default();
    let deadline = Deadline::standard().unwrap();
    let lease = InteractionLease::guarded(deadline, ()).unwrap();
    crate::ref_action::ref_action_single::execute_single_shot(
        crate::ref_action::ref_action_wait_context::RefActionWaitContext {
            adapter: &adapter,
            entry: &entry,
            ref_id: "@test:e1",
            context: &context,
        },
        ActionRequest::headed(Action::Click),
        deadline,
        &lease,
        |_, _, _| {
            assert!(adapter.0.focused.load(Ordering::SeqCst));
            assert_eq!(adapter.0.resolved.load(Ordering::SeqCst), 0);
            Ok(ActionResult::delivered_unverified("click"))
        },
    )
    .unwrap();
}
