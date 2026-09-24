use crate::adapter::NativeHandle;
use crate::{AdapterError, CommandContext, Deadline, PlatformAdapter, Point, ProcessId};
use std::time::Duration;

/// Longest the optional presentation-window lookup may block an action.
const WINDOW_LOOKUP_MAX_MS: u64 = 150;

/// Shortest slice worth spending on the lookup; below it the cue goes unbound.
const WINDOW_LOOKUP_MIN_MS: u64 = 20;

/// Where a physical-pointer cue lands and the exact live window that owns it.
///
/// Ref-resolved pointer targets carry their live window so the renderer can
/// bind the cue to it. Coordinate-only input has no window; the renderer still
/// shows that cue, unbound, rather than dropping it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PointerCue {
    pub(crate) point: Point,
    pub(crate) window: Option<(ProcessId, String)>,
}

impl PointerCue {
    /// The window both drag endpoints share. A drag that crosses windows has
    /// no single owner, so its drag and drop cues are shown unbound.
    pub(crate) fn shared_window(&self, other: &Self) -> Option<(ProcessId, String)> {
        self.window
            .clone()
            .filter(|window| other.window.as_ref() == Some(window))
    }

    pub(crate) fn instruction(
        &self,
        context: &CommandContext,
        click: bool,
    ) -> Result<super::CursorOverlayInstruction, AdapterError> {
        let instruction = super::CursorOverlayInstruction::new(
            self.point.clone(),
            context.cursor_overlay(),
            click,
        )?;
        Ok(match &self.window {
            Some(window) => instruction.with_window(window.clone()),
            None => instruction,
        })
    }
}

/// Looks up the exact live window hosting `handle` for cue presentation.
///
/// The lookup is optional, so it gets a quarter of the remaining action budget
/// capped at `WINDOW_LOOKUP_MAX_MS`, and is skipped when that slice would be
/// shorter than `WINDOW_LOOKUP_MIN_MS`. Adapters must honour the deadline they
/// receive. Disabled overlays, failures, and unknown windows all yield None.
pub(crate) fn presentation_window(
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    handle: &NativeHandle,
    pid: ProcessId,
    deadline: Deadline,
) -> Option<(ProcessId, String)> {
    if !context.cursor_overlay().is_enabled() {
        return None;
    }
    let budget = (deadline.remaining() / 4).min(Duration::from_millis(WINDOW_LOOKUP_MAX_MS));
    if budget < Duration::from_millis(WINDOW_LOOKUP_MIN_MS) {
        return None;
    }
    adapter
        .get_presentation_window_id(handle, deadline.capped(budget))
        .ok()
        .flatten()
        .map(|window| (pid, window))
}
