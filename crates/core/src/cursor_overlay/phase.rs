use serde::{Deserialize, Serialize};

/// Which half of an action a cursor instruction represents.
///
/// `Travel` is sent before an action and acknowledged once the cursor lands.
/// `Drag` arms pointer tracking and is acknowledged once the renderer is ready.
/// `Effect` is sent after dispatch and is fire-and-forget.
#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorPhase {
    #[default]
    Travel,
    Drag,
    Effect,
}
