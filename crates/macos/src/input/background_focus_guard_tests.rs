use super::*;
use std::collections::VecDeque;

const USER: i32 = 100;
const TARGET: i32 = 200;
const OTHER_APP: i32 = 300;

/// Scripted guard environment: `samples` is the frontmost value read at each
/// sample, and a successful restore makes the next unscripted read the user.
struct FakeIo {
    now: Duration,
    samples: VecDeque<Option<i32>>,
    restore_succeeds: bool,
    restores: Vec<i32>,
    restored: bool,
}

impl FakeIo {
    fn new(samples: &[Option<i32>], restore_succeeds: bool) -> Self {
        Self {
            now: Duration::ZERO,
            samples: samples.iter().copied().collect(),
            restore_succeeds,
            restores: Vec::new(),
            restored: false,
        }
    }
}

impl GuardIo for FakeIo {
    fn now(&mut self) -> Duration {
        self.now
    }

    fn frontmost(&mut self) -> Option<i32> {
        match self.samples.pop_front() {
            Some(sample) => sample,
            None if self.restored => Some(USER),
            None => Some(TARGET),
        }
    }

    fn restore(&mut self, pid: i32) -> bool {
        self.restores.push(pid);
        self.restored = self.restore_succeeds;
        self.restore_succeeds
    }

    fn sleep(&mut self, duration: Duration) {
        self.now += duration;
    }
}

#[test]
fn an_undisturbed_delivery_records_no_intervention() {
    let mut guard = FocusGuard::new(USER, TARGET);
    let mut io = FakeIo::new(&[Some(USER); 32], true);

    guard.watch(&mut io, GUARD_WINDOW);

    assert!(io.restores.is_empty());
    assert_eq!(io.now, GUARD_WINDOW);
    assert_eq!(
        guard.finish(Some(USER)),
        BackgroundFocusGuard {
            interventions: 0,
            restored: false,
            max_steal_ms: 0,
            yielded: false,
        }
    );
}

#[test]
fn a_steal_restores_only_the_users_app_and_measures_its_length() {
    let mut guard = FocusGuard::new(USER, TARGET);
    let mut io = FakeIo::new(&[Some(USER), Some(TARGET), Some(TARGET)], true);

    guard.watch(&mut io, GUARD_WINDOW);

    assert_eq!(io.restores, [USER, USER]);
    assert_eq!(
        guard.finish(Some(USER)),
        BackgroundFocusGuard {
            interventions: 2,
            restored: true,
            max_steal_ms: 50,
            yielded: false,
        }
    );
}

#[test]
fn a_target_that_keeps_stealing_gets_a_bounded_number_of_restores() {
    let mut guard = FocusGuard::new(USER, TARGET);
    let mut io = FakeIo::new(&[], false);

    guard.watch(&mut io, GUARD_WINDOW);

    assert_eq!(io.restores.len(), 3);
    let report = guard.finish(Some(TARGET));
    assert_eq!(report.interventions, 3);
    assert!(!report.restored);
    assert_eq!(report.max_steal_ms, 400);
}

#[test]
fn unreadable_samples_are_not_treated_as_a_steal() {
    let mut guard = FocusGuard::new(USER, TARGET);
    let mut io = FakeIo::new(&[None; 32], true);

    guard.watch(&mut io, GUARD_WINDOW);

    assert!(io.restores.is_empty());
    assert_eq!(guard.finish(None).interventions, 0);
}

#[test]
fn a_steal_that_ends_on_its_own_is_still_measured() {
    let mut guard = FocusGuard::new(USER, TARGET);
    let mut io = FakeIo::new(&[Some(TARGET), Some(USER)], false);

    guard.sample(&mut io);
    io.sleep(GUARD_POLL);
    guard.sample(&mut io);

    let report = guard.finish(Some(USER));
    assert_eq!(report.max_steal_ms, 25);
    assert_eq!(report.interventions, 1);
}

#[test]
fn a_switch_to_a_third_app_is_left_alone_and_ends_the_watch() {
    let mut guard = FocusGuard::new(USER, TARGET);
    let mut io = FakeIo::new(
        &[Some(USER), Some(OTHER_APP), Some(USER), Some(TARGET)],
        true,
    );

    guard.watch(&mut io, GUARD_WINDOW);

    assert!(
        io.restores.is_empty(),
        "a voluntary switch must not be undone"
    );
    assert_eq!(io.now, GUARD_POLL, "the watch stops at the switch");
    assert_eq!(
        guard.finish(Some(OTHER_APP)),
        BackgroundFocusGuard {
            interventions: 0,
            restored: false,
            max_steal_ms: 0,
            yielded: true,
        }
    );
}

#[test]
fn a_third_app_after_a_target_steal_ends_the_restores() {
    let mut guard = FocusGuard::new(USER, TARGET);
    let mut io = FakeIo::new(&[Some(TARGET), Some(OTHER_APP), Some(TARGET)], true);

    guard.watch(&mut io, GUARD_WINDOW);

    assert_eq!(io.restores, [USER]);
    let report = guard.finish(Some(OTHER_APP));
    assert!(report.yielded);
    assert!(!report.restored);
    assert_eq!(report.max_steal_ms, 25);
}

#[test]
fn a_target_that_was_already_frontmost_is_never_restored_away() {
    let mut guard = FocusGuard::new(TARGET, TARGET);
    let mut io = FakeIo::new(&[Some(TARGET); 32], true);

    guard.watch(&mut io, GUARD_WINDOW);

    assert!(io.restores.is_empty());
    assert_eq!(guard.finish(Some(TARGET)), BackgroundFocusGuard::default());
}
