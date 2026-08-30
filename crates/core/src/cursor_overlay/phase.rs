use serde::{Deserialize, Serialize};

/// Which half of an action a cursor instruction represents.
///
/// `Travel` is sent before the action dispatches and every platform renderer
/// must acknowledge it once the cursor lands. `Effect` is sent after dispatch
/// confirms and is fire-and-forget.
#[derive(Debug, Clone, Copy, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorPhase {
    #[default]
    Travel,
    Effect,
}
