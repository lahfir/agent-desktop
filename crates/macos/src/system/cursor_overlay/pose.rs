use agent_desktop_core::{
    CursorOverlayControl, CursorOverlayInstruction, CursorOverlayStyle, Point,
};
use std::time::{Duration, Instant};

/// Time the latest presented cue stays fully visible.
pub(super) const TARGET_POSE_IDLE_MS: u64 = 5_000;

/// Time the cue then takes to fade linearly to nothing before it is cleared.
pub(super) const TARGET_POSE_FADE_MS: u64 = 1_000;

/// Renderer-side memory for one agent cursor.
///
/// `at` is where the next travel animation starts. `pose_deadline` marks when
/// the latest presented pose starts fading; it is cleared once the fade ends.
/// Only a new presented instruction moves the deadline; target visibility
/// changes and Hide/Show lifecycle controls never extend, reset, or replay it.
#[derive(Default)]
pub(super) struct OverlayState {
    pub(super) style: CursorOverlayStyle,
    pub(super) at: Option<Point>,
    pub(super) pose_deadline: Option<Instant>,
}

enum PoseFade {
    Steady,
    Fading(f64),
    Expired,
}

impl OverlayState {
    /// Callers must also restore full native opacity for the new pose; the
    /// renderer does so at the start of every presentation.
    pub(super) fn record_target_pose(&mut self, now: Instant) {
        self.pose_deadline = Some(now + Duration::from_millis(TARGET_POSE_IDLE_MS));
    }

    fn pose_fade(&mut self, now: Instant) -> PoseFade {
        let Some(faded) = self
            .pose_deadline
            .and_then(|deadline| now.checked_duration_since(deadline))
        else {
            return PoseFade::Steady;
        };
        let fade = Duration::from_millis(TARGET_POSE_FADE_MS);
        if faded < fade {
            return PoseFade::Fading(1.0 - faded.as_secs_f64() / fade.as_secs_f64());
        }
        self.at = None;
        self.pose_deadline = None;
        PoseFade::Expired
    }
}

/// Advances the retained pose's idle fade by one renderer tick.
///
/// The fade is driven by these ticks rather than a blocking native loop so the
/// renderer keeps accepting instructions: an action arriving mid-fade is
/// presented (and acknowledged) immediately. `opacity` receives the fraction to
/// apply during the fade window; `expire` clears the pose once it ends. A tick
/// that lands after the whole window skips straight to `expire`. Callers skip
/// this while a drag is in progress so an active drag never fades mid-gesture.
pub(super) fn fade_pose(
    state: &mut OverlayState,
    now: Instant,
    opacity: impl FnOnce(f64),
    expire: impl FnOnce(),
) {
    match state.pose_fade(now) {
        PoseFade::Steady => {}
        PoseFade::Fading(alpha) => opacity(alpha),
        PoseFade::Expired => expire(),
    }
}

/// Selects the instruction a control asks the renderer to draw.
///
/// Only Present controls carry one, so enabling the overlay or a Show never
/// conjures a cursor on its own. A window-bound instruction follows its exact
/// target window's visibility. An unbound one (coordinate-only pointer input,
/// or a drag across windows) has no window to follow and is drawn as is. Both
/// get the same idle fade, so neither lingers.
pub(super) fn instruction_to_render(
    control: &CursorOverlayControl,
) -> Option<&CursorOverlayInstruction> {
    control.instruction()
}

/// Updates where the next travel animation starts.
///
/// Hide forgets the landing so a later travel never animates from a stale drag
/// origin, but it leaves `pose_deadline` and the native retained pose intact: a
/// Show before the fade ends brings the same cue back, at the opacity the fade
/// has reached, and after the fade the cue stays gone. Show changes nothing here.
pub(super) fn apply_landing_memory(
    control: &CursorOverlayControl,
    state: &mut OverlayState,
    instruction: Option<&CursorOverlayInstruction>,
) {
    if control.is_hide() {
        state.at = None;
        return;
    }
    if control.is_show() {
        return;
    }
    let Some(instruction) = instruction else {
        return;
    };
    state.at = Some(
        if instruction.phase() == agent_desktop_core::CursorPhase::Drag {
            instruction
                .drag_from()
                .unwrap_or(instruction.destination())
                .clone()
        } else {
            instruction.destination().clone()
        },
    );
}

#[cfg(test)]
#[path = "pose_tests.rs"]
mod tests;
