use agent_desktop_core::BackgroundFocusGuard;
use std::time::Duration;

/// How long the guard keeps watching after the last event: long enough to
/// catch an app that activates itself on the next run-loop turn.
pub(crate) const GUARD_WINDOW: Duration = Duration::from_millis(400);
pub(crate) const GUARD_POLL: Duration = Duration::from_millis(25);

/// Restores are retried a few times at most so a target that keeps
/// re-activating cannot turn the guard into a focus tug-of-war.
const MAX_INTERVENTIONS: u32 = 3;

/// Side effects the guard needs, injected so the decision logic runs in
/// tests against a scripted clock and frontmost sequence.
pub(crate) trait GuardIo {
    fn now(&mut self) -> Duration;
    fn frontmost(&mut self) -> Option<i32>;
    /// Makes `pid` frontmost again; `false` when that did not succeed.
    fn restore(&mut self, pid: i32) -> bool;
    fn sleep(&mut self, duration: Duration);
}

/// Tracks whether the delivery target displaced the user's frontmost app.
///
/// The guard only undoes a steal by the target itself: it restores the
/// user's app when the target became frontmost, never touches the target,
/// and gives up for good as soon as any third app becomes frontmost, because
/// that is the user (or something else they started) switching apps, which
/// the guard must not fight.
#[derive(Debug)]
pub(crate) struct FocusGuard {
    user_pid: i32,
    target_pid: i32,
    steal_started: Option<Duration>,
    max_steal: Duration,
    interventions: u32,
    yielded: bool,
}

impl FocusGuard {
    pub(crate) fn new(user_pid: i32, target_pid: i32) -> Self {
        Self {
            user_pid,
            target_pid,
            steal_started: None,
            max_steal: Duration::ZERO,
            interventions: 0,
            yielded: false,
        }
    }

    /// Takes one frontmost sample. Only the target counts as a steal, and
    /// only while it is not the user's own app. An unreadable sample is
    /// ignored, and nothing is sampled once the guard has yielded.
    pub(crate) fn sample(&mut self, io: &mut impl GuardIo) {
        if self.yielded {
            return;
        }
        let now = io.now();
        match io.frontmost() {
            Some(pid) if pid == self.user_pid => self.end_steal(now),
            Some(pid) if pid == self.target_pid => {
                let started = *self.steal_started.get_or_insert(now);
                self.max_steal = self.max_steal.max(now.saturating_sub(started));
                if self.interventions < MAX_INTERVENTIONS {
                    self.interventions += 1;
                    io.restore(self.user_pid);
                }
            }
            Some(_) => {
                self.end_steal(now);
                self.yielded = true;
            }
            None => {}
        }
    }

    /// Samples every [`GUARD_POLL`] until `window` has elapsed or the guard
    /// yields to a third app.
    pub(crate) fn watch(&mut self, io: &mut impl GuardIo, window: Duration) {
        let until = io.now() + window;
        loop {
            self.sample(io);
            let now = io.now();
            if self.yielded || now >= until {
                return;
            }
            io.sleep(GUARD_POLL.min(until - now));
        }
    }

    /// `restored` is true only when the guard intervened and the user's app
    /// is frontmost in the final sample.
    pub(crate) fn finish(&self, final_frontmost: Option<i32>) -> BackgroundFocusGuard {
        BackgroundFocusGuard {
            interventions: self.interventions,
            restored: self.interventions > 0 && final_frontmost == Some(self.user_pid),
            max_steal_ms: u64::try_from(self.max_steal.as_millis()).unwrap_or(u64::MAX),
            yielded: self.yielded,
        }
    }

    fn end_steal(&mut self, now: Duration) {
        if let Some(started) = self.steal_started.take() {
            self.max_steal = self.max_steal.max(now.saturating_sub(started));
        }
    }
}

#[cfg(test)]
#[path = "background_focus_guard_tests.rs"]
mod tests;
