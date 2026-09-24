use super::super::cursor_overlay_tests::entry;
use super::super::*;
use crate::adapter::{ActionOps, InputOps, NativeHandle, ObservationOps, SystemOps};
use crate::{Action, ActionResult, AdapterError, CursorOverlayControl, capability};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Answers the presentation-window lookup only once its deadline runs out, like
/// an app whose accessibility server stalls on the window attribute.
#[derive(Default)]
struct StalledWindowAdapter {
    lookup_budgets: Mutex<Vec<Duration>>,
    presented: Mutex<Vec<CursorOverlayControl>>,
}

impl ObservationOps for StalledWindowAdapter {
    fn get_presentation_window_id(
        &self,
        _handle: &NativeHandle,
        deadline: crate::Deadline,
    ) -> Result<Option<String>, AdapterError> {
        let budget = deadline.remaining();
        self.lookup_budgets.lock().unwrap().push(budget);
        std::thread::sleep(budget);
        Err(AdapterError::timeout("window lookup stalled"))
    }

    fn resolve_element_strict(
        &self,
        _entry: &RefEntry,
        _deadline: crate::Deadline,
    ) -> Result<NativeHandle, AdapterError> {
        Ok(NativeHandle::null())
    }

    crate::adapter::complete_live_observation!(
        "textfield",
        "Run",
        [capability::CLICK, capability::SET_VALUE]
    );
}

impl ActionOps for StalledWindowAdapter {
    fn execute_action(
        &self,
        _handle: &NativeHandle,
        _request: ActionRequest,
        _lease: &crate::InteractionLease,
    ) -> Result<ActionResult, AdapterError> {
        Ok(ActionResult::delivered_unverified("click"))
    }
}

impl InputOps for StalledWindowAdapter {}

impl SystemOps for StalledWindowAdapter {
    crate::adapter::guarded_interaction_lease!();
    crate::adapter::exact_window_focus!();

    fn update_cursor_overlay(&self, control: &CursorOverlayControl) -> Result<(), AdapterError> {
        self.presented.lock().unwrap().push(control.clone());
        Ok(())
    }
}

fn enabled_context() -> CommandContext {
    let config = crate::CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CommandContext::default().with_cursor_overlay_session("test-session", config)
}

#[test]
fn a_stalled_window_lookup_cannot_consume_the_action_budget() {
    let adapter = StalledWindowAdapter::default();
    let request = ActionRequest::headless(Action::Click).with_timeout_ms(Some(3_000));

    let started = Instant::now();
    let result = execute_entry_with_context(&adapter, &entry(), request, &enabled_context());
    let elapsed = started.elapsed();

    assert!(
        result.is_ok(),
        "presentation must not fail the action: {result:?}"
    );
    assert!(elapsed < Duration::from_millis(1_000), "{elapsed:?}");
    let budgets = adapter.lookup_budgets.lock().unwrap();
    assert!(!budgets.is_empty());
    assert!(
        budgets
            .iter()
            .all(|budget| *budget <= Duration::from_millis(150)),
        "{budgets:?}"
    );
    assert!(adapter.presented.lock().unwrap().is_empty());
}
