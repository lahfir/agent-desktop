//! Drag delivery/release-guard state.
//!
//! A `MouseInputEvent` batch is plain data - unlike macOS's `CGEvent`-backed
//! guard, constructing the corrective batch here cannot fail. The guard
//! arms immediately before the first committed injection (mouse-down) and
//! disarms only after the final release posts; its `Drop` is the sole place
//! that posts corrective input, and it always targets the **origin**, never
//! the destination, per
//! docs/solutions/best-practices/abort-state-guidance-multi-step-physical-input.md.
//! Cleanup is best-effort like every other post here: `SendInput`'s return
//! value is never treated as delivery evidence (A9-3), so an abort reports
//! `emergency_release_posted` without claiming the OS acknowledged it.

use agent_desktop_core::{AdapterError, DeliverySemantics};

use crate::input::mouse::{
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK,
};
use crate::input::mouse_coord::NormalizedPoint;
use crate::input::mouse_send::{MouseInputEvent, post_mouse_inputs};

pub(crate) struct DragReleaseGuard {
    origin: NormalizedPoint,
    armed: bool,
    delivered: usize,
}

impl DragReleaseGuard {
    pub(crate) fn new(origin: NormalizedPoint) -> Self {
        Self {
            origin,
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
            "Inspect the source and destination before retrying; the emergency release was posted without an OS acknowledgement",
        )
    }
}

impl Drop for DragReleaseGuard {
    fn drop(&mut self) {
        if !self.should_release() {
            return;
        }
        post_mouse_inputs(&[move_input(self.origin), button_input(MOUSEEVENTF_LEFTUP)]);
    }
}

fn move_input(point: NormalizedPoint) -> MouseInputEvent {
    let mut flags = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE;
    if point.virtual_desktop {
        flags |= MOUSEEVENTF_VIRTUALDESK;
    }
    MouseInputEvent {
        dx: point.x,
        dy: point.y,
        mouse_data: 0,
        flags,
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
#[path = "drag_state_tests.rs"]
mod tests;
