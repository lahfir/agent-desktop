//! Click delivery/release-guard state.
//!
//! Arms immediately before each mouse-down and disarms only after the
//! matching up posts; `Drop` is the sole place that posts a corrective up
//! when the sequence aborts mid-click. Cleanup is best-effort like every
//! other post here: `SendInput`'s return value is never treated as delivery
//! evidence (A9-3).

use agent_desktop_core::{AdapterError, DeliverySemantics};

use crate::input::mouse_send::{MouseInputEvent, post_mouse_inputs};

pub(crate) struct ClickReleaseGuard {
    up_flag: u32,
    armed: bool,
    delivered: usize,
}

impl ClickReleaseGuard {
    pub(crate) fn new(up_flag: u32) -> Self {
        Self {
            up_flag,
            armed: false,
            delivered: 0,
        }
    }

    pub(crate) fn arm(&mut self) {
        self.armed = true;
    }

    pub(crate) fn disarm(&mut self) {
        self.armed = false;
    }

    pub(crate) fn mark_delivered(&mut self) {
        self.delivered = self.delivered.saturating_add(1);
    }

    pub(crate) fn delivered_units(&self) -> usize {
        self.delivered
    }

    pub(crate) fn should_release(&self) -> bool {
        self.armed
    }

    pub(crate) fn enrich_error(&self, error: AdapterError) -> AdapterError {
        if self.delivered_units() == 0 {
            return error.with_disposition(DeliverySemantics::not_delivered());
        }
        let error = error.with_disposition(DeliverySemantics::delivered_unverified());
        let mut details = error
            .details
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        if let Some(map) = details.as_object_mut() {
            map.insert("delivered_events".into(), self.delivered_units().into());
            if self.should_release() {
                map.insert("emergency_release_posted".into(), true.into());
                map.insert("emergency_release_acknowledged".into(), false.into());
            }
        }
        error.with_details(details).with_suggestion(
            "Inspect whether a mouse button may still be held before retrying; the emergency release was posted without an OS acknowledgement",
        )
    }
}

impl Drop for ClickReleaseGuard {
    fn drop(&mut self) {
        if !self.should_release() {
            return;
        }
        post_mouse_inputs(&[button_input(self.up_flag)]);
    }
}

fn button_input(flag: u32) -> MouseInputEvent {
    MouseInputEvent {
        dx: 0,
        dy: 0,
        mouse_data: 0,
        flags: flag,
    }
}

#[cfg(all(test, target_os = "windows"))]
#[path = "mouse_click_guard_tests.rs"]
mod tests;
