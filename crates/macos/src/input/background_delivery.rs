use agent_desktop_core::{AdapterError, BackgroundPointerReport, Deadline, ErrorCode, WindowInfo};
use core_graphics::event::CGEvent;
use std::time::{Duration, Instant};

use crate::actions::DeliveryTracker;
use crate::input::background_activation::focus_target_window;
use crate::input::background_focus_guard::{FocusGuard, GUARD_WINDOW, GuardIo};
use crate::input::background_frontmost::frontmost_pid;
use crate::input::background_layers::BackgroundLayers;
use crate::input::prepared_event::PreparedEvent;
use crate::input::skylight;

/// Pause after the focus record so the target processes it before the
/// input events arrive (background-computer-use waits 50 ms).
const ACTIVATION_SETTLE: Duration = Duration::from_millis(50);
/// Settle time before the after-sample when the focus guard is off.
const FOCUS_SETTLE: Duration = Duration::from_millis(100);

/// A background delivery whose events are all built. Everything that can
/// fail must fail before one of these exists, so [`run`] never reports a
/// partial delivery as not delivered.
pub(crate) struct Prepared {
    pub(crate) pid: libc::pid_t,
    pub(crate) window_number: u32,
    pub(crate) layers: BackgroundLayers,
    /// Events in posting order.
    pub(crate) events: Vec<PreparedEvent>,
    pub(crate) degradations: Vec<String>,
}

/// Everything [`run`] does to the outside world: the clock and frontmost
/// reads of [`GuardIo`] plus the window-server record and the two posting
/// paths. Production uses [`SystemDeliveryIo`]; tests script a fake so the
/// ordering, fallback, and deadline behavior are observable without posting.
pub(crate) trait DeliveryIo: GuardIo {
    /// Sends the target-only focus record; `Err` carries the degradation
    /// reason.
    fn focus_target_window(&mut self, pid: libc::pid_t, window_number: u32) -> Result<(), String>;
    /// Posts through `SLEventPostToPid`; `false` means the symbol is missing
    /// and nothing was posted.
    fn post_skylight(&mut self, pid: libc::pid_t, event: &CGEvent) -> bool;
    fn post_to_pid(&mut self, pid: libc::pid_t, event: &CGEvent);
}

/// Posts a prepared delivery to one window's process.
///
/// Order: sample the frontmost app, send the target-only focus record when
/// the `activate` layer asks for it, post the events through exactly one
/// path each, then let the focus guard watch for a steal.
///
/// The deadline is checked before every event that starts something new (a
/// move, a button or key down, a wheel chunk) and once more after the last
/// event. An event that completes a press already posted is always sent, so
/// nothing is left held down, but a delivery that finishes past the budget
/// still fails. When the budget runs out the delivery stops there with
/// `TIMEOUT`: not delivered (safe to retry) if no input event was posted yet,
/// otherwise delivered-unverified (unsafe to retry) with the posted and
/// planned event counts in the details. The focus record alone does not count
/// as delivery: it changes no content and resending it is idempotent.
pub(crate) fn run(
    prepared: Prepared,
    deadline: Deadline,
    io: &mut impl DeliveryIo,
) -> Result<BackgroundPointerReport, AdapterError> {
    let Prepared {
        pid,
        window_number,
        layers,
        events,
        mut degradations,
    } = prepared;
    let budget_end = io.now() + deadline.remaining();
    let mut delivery = DeliveryTracker::default();
    let planned = events.len();

    let before = io.frontmost();
    let mut guard = start_guard(layers, before, pid, &mut degradations);
    if expired(io, budget_end) {
        return Err(timeout(deadline, delivery, planned));
    }

    if layers.activate {
        match io.focus_target_window(pid, window_number) {
            Ok(()) => pause(io, budget_end, ACTIVATION_SETTLE),
            Err(reason) => degradations.push(reason),
        }
        sample(&mut guard, io);
    }

    for prepared_event in &events {
        if !prepared_event.completes_press && expired(io, budget_end) {
            return Err(timeout(deadline, delivery, planned));
        }
        post(
            pid,
            &prepared_event.event,
            layers.skylight,
            io,
            &mut degradations,
        );
        delivery.mark_delivered();
        sample(&mut guard, io);
        pause(io, budget_end, prepared_event.pause_after);
    }

    if expired(io, budget_end) {
        return Err(timeout(deadline, delivery, planned));
    }

    match guard.as_mut() {
        Some(guard) => {
            let window = GUARD_WINDOW.min(budget_end.saturating_sub(io.now()));
            guard.watch(io, window);
        }
        None => pause(io, budget_end, FOCUS_SETTLE),
    }

    let after = io.frontmost();
    Ok(BackgroundPointerReport {
        frontmost_pid_before: before.and_then(to_process_id),
        frontmost_pid_after: after.and_then(to_process_id),
        layers: layers.names(),
        degradations,
        focus_guard: guard.map(|guard| guard.finish(after)),
    })
}

/// The CoreGraphics window number behind a `w-<number>` window id.
pub(crate) fn window_number(window: &WindowInfo) -> Result<u32, AdapterError> {
    crate::system::window_resolve::parse_window_number(&window.id)
        .and_then(|number| u32::try_from(number).ok())
        .ok_or_else(|| {
            AdapterError::new(
                ErrorCode::InvalidArgs,
                format!("'{}' is not a window id", window.id),
            )
        })
}

pub(crate) fn note_once(degradations: &mut Vec<String>, reason: &str) {
    if !degradations.iter().any(|existing| existing == reason) {
        degradations.push(reason.to_string());
    }
}

fn expired(io: &mut impl DeliveryIo, budget_end: Duration) -> bool {
    io.now() >= budget_end
}

/// Sleeps for `duration`, cut short at the end of the budget.
fn pause(io: &mut impl DeliveryIo, budget_end: Duration, duration: Duration) {
    let remaining = budget_end.saturating_sub(io.now());
    io.sleep(duration.min(remaining));
}

/// Adds the event counts to the deadline's own details (`kind`,
/// `timeout_ms`, `elapsed_ms`) instead of replacing them.
fn timeout(deadline: Deadline, delivery: DeliveryTracker, planned: usize) -> AdapterError {
    let mut error = deadline.timeout_error();
    let mut details = error
        .details
        .take()
        .unwrap_or_else(|| serde_json::json!({}));
    details["delivered_events"] = delivery.delivered_units().into();
    details["planned_events"] = planned.into();
    delivery.annotate(error.with_details(details))
}

/// Posts through exactly one path: SkyLight when requested and available,
/// otherwise `CGEventPostToPid`, so an event is never delivered twice.
fn post(
    pid: libc::pid_t,
    event: &CGEvent,
    use_skylight: bool,
    io: &mut impl DeliveryIo,
    degradations: &mut Vec<String>,
) {
    if use_skylight {
        if io.post_skylight(pid, event) {
            return;
        }
        note_once(degradations, "skylight:SLEventPostToPid_unavailable");
    }
    io.post_to_pid(pid, event);
}

/// The guard needs to know whose frontmost position to protect; the target
/// pid tells a self-activation apart from the user switching apps.
fn start_guard(
    layers: BackgroundLayers,
    before: Option<i32>,
    target_pid: libc::pid_t,
    degradations: &mut Vec<String>,
) -> Option<FocusGuard> {
    if !layers.guard {
        return None;
    }
    if before.is_none() {
        degradations.push("guard:frontmost_unknown".to_string());
    }
    before.map(|user_pid| FocusGuard::new(user_pid, target_pid))
}

fn sample(guard: &mut Option<FocusGuard>, io: &mut impl DeliveryIo) {
    if let Some(guard) = guard.as_mut() {
        guard.sample(io);
    }
}

fn to_process_id(pid: i32) -> Option<agent_desktop_core::ProcessId> {
    crate::system::process_identity::from_pid_t(pid).ok()
}

/// The real clock, frontmost reads, and posting paths.
pub(crate) struct SystemDeliveryIo {
    started: Instant,
    deadline: Deadline,
    restore_unavailable: bool,
}

impl SystemDeliveryIo {
    pub(crate) fn new(deadline: Deadline) -> Self {
        Self {
            started: Instant::now(),
            deadline,
            restore_unavailable: false,
        }
    }

    /// Adds the degradation for a guard restore that could not even be
    /// attempted because the SkyLight symbols are missing.
    pub(crate) fn note_degradations(&self, degradations: &mut Vec<String>) {
        if self.restore_unavailable {
            note_once(degradations, "guard:restore_unavailable");
        }
    }
}

impl GuardIo for SystemDeliveryIo {
    fn now(&mut self) -> Duration {
        self.started.elapsed()
    }

    fn frontmost(&mut self) -> Option<i32> {
        frontmost_pid(self.deadline)
    }

    fn restore(&mut self, pid: i32) -> bool {
        let restored = skylight::restore_front_process(pid);
        self.restore_unavailable |= restored.is_none();
        restored.unwrap_or(false)
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

impl DeliveryIo for SystemDeliveryIo {
    fn focus_target_window(&mut self, pid: libc::pid_t, window_number: u32) -> Result<(), String> {
        focus_target_window(pid, window_number)
    }

    fn post_skylight(&mut self, pid: libc::pid_t, event: &CGEvent) -> bool {
        skylight::post_to_pid(pid, event)
    }

    fn post_to_pid(&mut self, pid: libc::pid_t, event: &CGEvent) {
        event.post_to_pid(pid);
    }
}

#[cfg(test)]
#[path = "background_delivery_tests.rs"]
mod tests;
