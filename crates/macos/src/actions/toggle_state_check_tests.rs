use std::collections::VecDeque;

use agent_desktop_core::{
    ActionStep, AdapterError, DeliverySemantics, ErrorCode, action_step_outcome::ActionStepOutcome,
};

use super::{CheckTarget, run_check};

struct Scripted {
    reads: VecDeque<Option<bool>>,
    settles: VecDeque<Option<bool>>,
    settable: bool,
    writes: usize,
    clicks: usize,
}

impl Scripted {
    fn new(reads: &[Option<bool>], settles: &[Option<bool>], settable: bool) -> Self {
        Self {
            reads: reads.iter().copied().collect(),
            settles: settles.iter().copied().collect(),
            settable,
            writes: 0,
            clicks: 0,
        }
    }
}

impl CheckTarget for Scripted {
    fn checked(&mut self) -> Result<Option<bool>, AdapterError> {
        Ok(self.reads.pop_front().expect("unexpected state read"))
    }

    fn value_settable(&mut self) -> Result<bool, AdapterError> {
        Ok(self.settable)
    }

    fn write_value(&mut self, _want_checked: bool) -> Result<bool, AdapterError> {
        self.writes += 1;
        Ok(true)
    }

    fn settle(&mut self, _want_checked: bool) -> Result<Option<bool>, AdapterError> {
        Ok(self.settles.pop_front().expect("unexpected settle"))
    }

    fn click(&mut self) -> Result<Vec<ActionStep>, AdapterError> {
        self.clicks += 1;
        Ok(vec![ActionStep::succeeded("AXPress").with_verified(false)])
    }
}

fn labels(steps: &[ActionStep]) -> Vec<(String, Option<bool>)> {
    steps
        .iter()
        .map(|step| (step.label().to_string(), step.verified()))
        .collect()
}

#[test]
fn already_checked_sends_nothing() {
    let mut target = Scripted::new(&[Some(true)], &[], true);
    let steps = run_check(&mut target, true, true).unwrap();
    assert_eq!(labels(&steps), [("AlreadyInState".into(), Some(true))]);
    assert_eq!((target.writes, target.clicks), (0, 0));
}

#[test]
fn already_unchecked_sends_nothing() {
    let mut target = Scripted::new(&[Some(false)], &[], true);
    let steps = run_check(&mut target, false, true).unwrap();
    assert_eq!(labels(&steps), [("AlreadyInState".into(), Some(true))]);
    assert_eq!((target.writes, target.clicks), (0, 0));
}

#[test]
fn a_value_write_that_lands_does_not_click() {
    let mut target = Scripted::new(&[Some(false)], &[Some(true)], true);
    let steps = run_check(&mut target, true, true).unwrap();
    assert_eq!(labels(&steps), [("AXValue".into(), Some(true))]);
    assert_eq!((target.writes, target.clicks), (1, 0));
}

#[test]
fn an_accepted_write_that_changes_nothing_falls_back_to_one_click() {
    let mut target = Scripted::new(
        &[Some(false), Some(false)],
        &[Some(false), Some(true)],
        true,
    );
    let steps = run_check(&mut target, true, true).unwrap();
    assert_eq!(
        labels(&steps),
        [("AXValue".into(), None), ("AXPress".into(), Some(true))]
    );
    assert!(matches!(steps[0].outcome, ActionStepOutcome::Attempted));
    assert_eq!((target.writes, target.clicks), (1, 1));
}

#[test]
fn a_write_that_lands_late_is_not_clicked_back() {
    let mut target = Scripted::new(&[Some(false), Some(true)], &[Some(false)], true);
    let steps = run_check(&mut target, true, true).unwrap();
    assert_eq!(labels(&steps), [("AXValue".into(), Some(true))]);
    assert_eq!(target.clicks, 0);
}

#[test]
fn an_indeterminate_state_after_the_write_stops_without_clicking() {
    let mut target = Scripted::new(&[Some(false), None], &[Some(false)], true);
    let error = run_check(&mut target, true, true).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(target.clicks, 0);
}

#[test]
fn a_click_that_does_not_reach_the_state_fails_as_delivered_unverified() {
    let mut target = Scripted::new(
        &[Some(false), Some(false)],
        &[Some(false), Some(false)],
        true,
    );
    let error = run_check(&mut target, true, true).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(target.clicks, 1);
}

#[test]
fn an_element_without_a_settable_value_is_clicked_directly() {
    let mut target = Scripted::new(&[Some(true)], &[Some(false)], false);
    let steps = run_check(&mut target, false, true).unwrap();
    assert_eq!(labels(&steps), [("AXPress".into(), Some(true))]);
    assert_eq!((target.writes, target.clicks), (0, 1));
}

#[test]
fn unchecking_a_radio_that_ignores_the_write_does_not_press_it() {
    let mut target = Scripted::new(&[Some(true), Some(true)], &[Some(true)], true);
    let error = run_check(&mut target, false, false).unwrap_err();
    assert_eq!(error.code, ErrorCode::ActionFailed);
    assert_eq!(error.disposition, DeliverySemantics::delivered_unverified());
    assert_eq!(target.clicks, 0);
}
