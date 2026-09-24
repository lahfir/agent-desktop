use super::*;
use agent_desktop_core::{CursorOverlayConfig, ProcessId};

fn target(x: f64) -> CursorOverlayInstruction {
    let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
    CursorOverlayInstruction::new(Point { x, y: 300.0 }, &config, false)
        .expect("valid instruction")
        .with_window((ProcessId::new(10), "w-42".into()))
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

/// Ticks the pose once and reports the opacity it set, if any, and whether the
/// tick expired the pose.
fn tick(state: &mut OverlayState, at: Instant) -> (Option<f64>, bool) {
    let mut opacity = None;
    let mut expired = false;
    fade_pose(state, at, |alpha| opacity = Some(alpha), || expired = true);
    (opacity, expired)
}

fn expiries_at(state: &mut OverlayState, at: Instant) -> usize {
    usize::from(tick(state, at).1)
}

#[test]
fn only_present_controls_select_an_instruction_and_unbound_cues_still_render() {
    let enable = CursorOverlayControl::enable("run-enable".into(), CursorOverlayStyle::default());
    let show = CursorOverlayControl::show("run-enable".into());
    let unbound = CursorOverlayControl::present("run-enable".into(), {
        let config = CursorOverlayConfig::enabled(None, 6).expect("valid config");
        CursorOverlayInstruction::new(Point { x: 400.0, y: 300.0 }, &config, false)
            .expect("valid instruction")
            .with_phase(agent_desktop_core::CursorPhase::Drag)
    });
    let target = CursorOverlayControl::present("run-enable".into(), target(400.0));

    assert!(instruction_to_render(&enable).is_none());
    assert!(instruction_to_render(&show).is_none());
    assert!(instruction_to_render(&unbound).is_some());
    assert!(instruction_to_render(&target).is_some());
}

#[test]
fn hide_then_show_before_expiry_restores_the_pose_on_its_original_deadline() {
    let now = Instant::now();
    let mut state = OverlayState::default();
    let shown = target(400.0);
    state.record_target_pose(now);
    apply_landing_memory(
        &CursorOverlayControl::present("run-bracket".into(), shown.clone()),
        &mut state,
        Some(&shown),
    );

    assert_eq!(tick(&mut state, now + ms(1_000)), (None, false));
    apply_landing_memory(
        &CursorOverlayControl::hide("run-bracket".into()),
        &mut state,
        None,
    );
    assert_eq!(tick(&mut state, now + ms(2_000)), (None, false));
    apply_landing_memory(
        &CursorOverlayControl::show("run-bracket".into()),
        &mut state,
        None,
    );

    assert_eq!(state.pose_deadline, Some(now + ms(TARGET_POSE_IDLE_MS)));
    assert_eq!(
        tick(&mut state, now + ms(TARGET_POSE_IDLE_MS - 1)),
        (None, false)
    );
    let (opacity, expired) = tick(&mut state, now + ms(TARGET_POSE_IDLE_MS + 500));
    assert!(!expired);
    assert!((opacity.expect("fades on the original schedule") - 0.5).abs() < 1e-9);
    assert_eq!(
        expiries_at(
            &mut state,
            now + ms(TARGET_POSE_IDLE_MS + TARGET_POSE_FADE_MS)
        ),
        1
    );
}

#[test]
fn target_pose_stays_fully_visible_for_five_seconds() {
    assert_eq!(TARGET_POSE_IDLE_MS, 5_000);
    let now = Instant::now();
    let mut state = OverlayState::default();
    state.record_target_pose(now);

    assert_eq!(tick(&mut state, now), (None, false));
    assert_eq!(
        tick(&mut state, now + ms(TARGET_POSE_IDLE_MS - 1)),
        (None, false)
    );
    assert!(state.pose_deadline.is_some());
}

#[test]
fn target_pose_fades_linearly_over_one_second_then_expires_once() {
    assert_eq!(TARGET_POSE_FADE_MS, 1_000);
    let now = Instant::now();
    let mut state = OverlayState::default();
    state.record_target_pose(now);

    assert_eq!(
        tick(&mut state, now + ms(TARGET_POSE_IDLE_MS)),
        (Some(1.0), false)
    );
    let (quarter, expired) = tick(&mut state, now + ms(TARGET_POSE_IDLE_MS + 250));
    assert!(!expired);
    assert!((quarter.expect("fading") - 0.75).abs() < 1e-9);
    let (late, expired) = tick(&mut state, now + ms(TARGET_POSE_IDLE_MS + 900));
    assert!(!expired);
    assert!((late.expect("fading") - 0.1).abs() < 1e-9);

    let end = now + ms(TARGET_POSE_IDLE_MS + TARGET_POSE_FADE_MS);
    assert_eq!(tick(&mut state, end), (None, true));
    assert_eq!(state.pose_deadline, None);
    assert_eq!(tick(&mut state, end + ms(1)), (None, false));
}

#[test]
fn a_new_target_mid_fade_restarts_the_full_visibility_window() {
    let now = Instant::now();
    let mut state = OverlayState::default();
    state.record_target_pose(now);
    let mid_fade = now + ms(TARGET_POSE_IDLE_MS + 500);
    assert!(tick(&mut state, mid_fade).0.is_some());

    state.record_target_pose(mid_fade);

    assert_eq!(tick(&mut state, mid_fade), (None, false));
    assert_eq!(
        tick(&mut state, mid_fade + ms(TARGET_POSE_IDLE_MS - 1)),
        (None, false)
    );
    assert_eq!(
        expiries_at(
            &mut state,
            now + ms(TARGET_POSE_IDLE_MS + TARGET_POSE_FADE_MS)
        ),
        0
    );
    assert_eq!(
        expiries_at(
            &mut state,
            mid_fade + ms(TARGET_POSE_IDLE_MS + TARGET_POSE_FADE_MS)
        ),
        1
    );
}

#[test]
fn a_skipped_fade_window_expires_immediately_without_an_opacity_step() {
    let now = Instant::now();
    let mut state = OverlayState::default();
    state.record_target_pose(now);

    assert_eq!(
        tick(&mut state, now + ms(TARGET_POSE_IDLE_MS + 60_000)),
        (None, true)
    );
}

#[test]
fn hide_forgets_the_landing_but_keeps_the_idle_deadline() {
    let now = Instant::now();
    let mut state = OverlayState::default();
    let shown = target(400.0);
    let present = CursorOverlayControl::present("run-hide".into(), shown.clone());
    state.record_target_pose(now);
    apply_landing_memory(&present, &mut state, Some(&shown));

    apply_landing_memory(
        &CursorOverlayControl::hide("run-hide".into()),
        &mut state,
        None,
    );

    assert_eq!(state.at, None);
    assert_eq!(state.pose_deadline, Some(now + ms(TARGET_POSE_IDLE_MS)));
    assert_eq!(
        tick(&mut state, now + ms(TARGET_POSE_IDLE_MS - 1)),
        (None, false)
    );
    assert!(
        tick(&mut state, now + ms(TARGET_POSE_IDLE_MS + 500))
            .0
            .is_some()
    );
    assert_eq!(
        expiries_at(
            &mut state,
            now + ms(TARGET_POSE_IDLE_MS + TARGET_POSE_FADE_MS)
        ),
        1
    );
}

#[test]
fn show_during_or_after_the_fade_never_extends_it() {
    let now = Instant::now();
    let mut state = OverlayState::default();
    state.record_target_pose(now);
    let show = CursorOverlayControl::show("run-show".into());

    apply_landing_memory(
        &CursorOverlayControl::hide("run-show".into()),
        &mut state,
        None,
    );
    apply_landing_memory(&show, &mut state, None);
    let (opacity, _) = tick(&mut state, now + ms(TARGET_POSE_IDLE_MS + 500));
    assert!((opacity.expect("still fading") - 0.5).abs() < 1e-9);

    apply_landing_memory(&show, &mut state, None);
    assert_eq!(
        expiries_at(
            &mut state,
            now + ms(TARGET_POSE_IDLE_MS + TARGET_POSE_FADE_MS)
        ),
        1
    );

    apply_landing_memory(&show, &mut state, None);
    assert_eq!(state.pose_deadline, None);
    assert_eq!(
        tick(
            &mut state,
            now + ms(2 * (TARGET_POSE_IDLE_MS + TARGET_POSE_FADE_MS))
        ),
        (None, false)
    );
}
